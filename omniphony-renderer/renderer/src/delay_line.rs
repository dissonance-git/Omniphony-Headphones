//! Per-speaker fractional delay line with linear-interpolated read pointer.
//!
//! # Design
//!
//! Each `DelayLine` holds a fixed-size circular buffer sized for 100 ms at the
//! renderer's sample rate.  The read pointer is fractional and ramps toward the
//! target at a capped velocity of **1 delay-sample per output sample**, so a
//! 100 ms delay change takes at most 100 ms to complete with no discontinuity.
//!
//! Fractional positions are resolved with linear interpolation between the two
//! neighbouring buffer slots.

/// Maximum ramp speed: delay changes by at most this many samples per output sample.
/// At this rate a 100 ms change at 48 kHz (4 800 samples) completes in 100 ms.
const RAMP_RATE: f32 = 1.0;

pub struct DelayLine {
    /// Circular buffer, zero-initialised.  Size = max_delay_samples + 2.
    /// The +2 gives one extra slot for the linear-interpolation upper neighbour
    /// and one slot of safety margin.
    buf: Vec<f32>,

    /// Next write position (advances by 1 each sample, wraps at buf.len()).
    write_pos: usize,

    /// Current fractional delay in samples — the actual read offset used this
    /// sample.  Ramps toward `target` at ≤ RAMP_RATE per sample.
    current: f32,

    /// Target delay in samples, pre-computed from `delay_ms × sample_rate / 1000`.
    /// Updated by `set_target_ms`; never changes between calls.
    target: f32,

    /// Maximum delay change applied per output sample. Generic callers use the
    /// historical one-sample rate; authored source motion may choose the rate
    /// that reaches the next source-time boundary over a known sample span.
    ramp_rate: f32,
}

impl DelayLine {
    /// Allocate a delay line capable of holding up to `max_delay_samples` of
    /// history.  The buffer is zeroed so early reads produce silence.
    pub fn new(max_delay_samples: usize) -> Self {
        Self {
            buf: vec![0.0f32; max_delay_samples + 2],
            write_pos: 0,
            current: 0.0,
            target: 0.0,
            ramp_rate: RAMP_RATE,
        }
    }

    /// Set the target delay from milliseconds + sample rate.
    ///
    /// The conversion (`ms × sr / 1000`) is done **once here**, so `process`
    /// never performs it in the hot loop.  Clamped to `[0, max_delay_samples]`.
    pub fn set_target_ms(&mut self, delay_ms: f32, sample_rate: u32) {
        let max = (self.buf.len() - 2) as f32;
        self.target = (delay_ms * sample_rate as f32 / 1000.0).clamp(0.0, max);
        self.ramp_rate = RAMP_RATE;
    }

    /// Set a delay target whose transition belongs to an authored source-time
    /// span. Zero samples is an explicit metadata jump and snaps at the block
    /// boundary; a positive span chooses the per-sample rate required to reach
    /// the target during that span. This is separate from generic smoothing so
    /// transport callback size never becomes the motion clock.
    pub fn set_target_ms_over_samples(
        &mut self,
        delay_ms: f32,
        sample_rate: u32,
        ramp_samples: u32,
    ) {
        let max = (self.buf.len() - 2) as f32;
        self.target = (delay_ms * sample_rate as f32 / 1000.0).clamp(0.0, max);
        if ramp_samples == 0 {
            self.current = self.target;
            self.ramp_rate = RAMP_RATE;
        } else {
            self.ramp_rate = (self.target - self.current).abs() / ramp_samples as f32;
        }
    }

    /// Returns `true` if this delay line is a no-op (target and current are 0).
    #[inline]
    pub fn is_bypass(&self) -> bool {
        self.target == 0.0 && self.current == 0.0
    }

    /// Keep the ring warm without doing the fractional read.
    ///
    /// While [`is_bypass`](Self::is_bypass) holds, `process` is the identity.
    /// The write still matters because a later non-zero delay must be able to
    /// read audio that passed while the line was bypassed.
    #[inline]
    pub fn push_history(&mut self, input: f32) {
        debug_assert!(
            self.is_bypass(),
            "push_history is only the identity while bypassed",
        );
        self.buf[self.write_pos] = input;
        self.write_pos = (self.write_pos + 1) % self.buf.len();
    }

    /// Reset stream-lifetime history in place. Capacity is retained so a
    /// decoder seek/track restart cannot leak old delayed samples into the new
    /// stream and does not allocate or free on the realtime thread.
    pub fn reset_runtime_state(&mut self) {
        self.buf.fill(0.0);
        self.write_pos = 0;
        self.current = 0.0;
        self.target = 0.0;
        self.ramp_rate = RAMP_RATE;
    }

    /// Process one sample through the delay line.
    ///
    /// Write `input` into the buffer, ramp the read pointer one step toward the
    /// target, then return the linearly-interpolated sample at the current read
    /// position.
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let cap = self.buf.len();

        // Write.
        self.buf[self.write_pos] = input;

        // Ramp current toward target. Generic delay changes use RAMP_RATE;
        // authored source motion may provide a source-time-derived rate.
        let delta = self.target - self.current;
        if delta.abs() <= self.ramp_rate {
            self.current = self.target;
        } else if self.ramp_rate > 0.0 {
            self.current += self.ramp_rate * delta.signum();
        }

        // Fractional read (linear interpolation).
        let mut read_f = (self.write_pos as f32 - self.current).rem_euclid(cap as f32);
        // f32 edge: rem_euclid of a tiny negative (current a hair above
        // write_pos) rounds to exactly `cap`, which floor()s to an
        // out-of-bounds index. `cap` is the same position as 0 — wrap it.
        if read_f >= cap as f32 {
            read_f = 0.0;
        }
        let i0 = read_f as usize;
        let i1 = (i0 + 1) % cap;
        let frac = read_f - i0 as f32;
        let output = self.buf[i0] + frac * (self.buf[i1] - self.buf[i0]);

        // Advance write pointer.
        self.write_pos = (self.write_pos + 1) % cap;

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: a fractional target a hair above an integer write position
    /// makes `(write_pos - current)` a tiny negative; `rem_euclid(cap)` then
    /// rounds to exactly `cap` in f32 and the floor()ed read index lands out
    /// of bounds. Sweep fractional targets right above 5 samples through full
    /// buffer laps — this panicked before the wrap guard.
    #[test]
    fn bypass_history_matches_full_processing_when_delay_turns_on() {
        let mut fast = DelayLine::new(32);
        let mut reference = DelayLine::new(32);
        for i in 0..24 {
            let x = ((i * 7 % 17) as f32 - 8.0) / 8.0;
            fast.push_history(x);
            assert_eq!(reference.process(x).to_bits(), x.to_bits());
        }
        fast.set_target_ms(4.0 / 48.0, 48_000);
        reference.set_target_ms(4.0 / 48.0, 48_000);
        for i in 0..32 {
            let x = ((i * 11 % 19) as f32 - 9.0) / 9.0;
            assert_eq!(fast.process(x).to_bits(), reference.process(x).to_bits());
        }
    }

    #[test]
    fn authored_timed_target_uses_requested_motion_span_and_jump_semantics() {
        let mut dl = DelayLine::new(64);
        dl.set_target_ms_over_samples(12.0 / 48.0, 48_000, 6);
        assert!((dl.ramp_rate - 2.0).abs() < 1.0e-6);
        for _ in 0..5 {
            dl.process(0.0);
        }
        assert!(dl.current < 12.0);
        dl.process(0.0);
        assert!((dl.current - 12.0).abs() < 1.0e-6);

        dl.set_target_ms_over_samples(3.0 / 48.0, 48_000, 0);
        assert!((dl.current - 3.0).abs() < 1.0e-6);
        assert!((dl.target - 3.0).abs() < 1.0e-6);
    }

    #[test]
    fn fractional_delay_just_above_write_pos_does_not_panic() {
        for k in 0..400 {
            let mut dl = DelayLine::new(144);
            // Targets densely covering (5.0, 5.0 + ~1e-5) samples.
            let target_samples = 5.0f32 + k as f32 * 2.5e-8;
            dl.set_target_ms(target_samples / 48.0, 48_000);
            for i in 0..600 {
                let y = dl.process((i % 7) as f32 - 3.0);
                assert!(y.is_finite());
            }
        }
    }
}
