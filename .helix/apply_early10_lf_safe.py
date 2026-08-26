from pathlib import Path

p = Path('omniphony-renderer/orender_engine/src/music_early_reflections.rs')
s = p.read_text()

def once(old: str, new: str, label: str) -> None:
    global s
    n = s.count(old)
    if n != 1:
        raise RuntimeError(f'{label}: expected one anchor, found {n}')
    s = s.replace(old, new, 1)

once(
"use renderer::binaural::measured::MeasuredHrirData;\nuse renderer::binaural::reflections::{self, NUM_REFLECTIONS};",
"use renderer::binaural::measured::MeasuredHrirData;\nuse renderer::binaural::reflections::{self, NUM_REFLECTIONS};\nuse renderer::crossover::filter::{\n    BiquadCoeffs, BiquadState, biquad, butterworth2_hp, butterworth2_lp,\n};",
'import crossover helpers')

once(
"const TRANSIENT_MAX_GAIN_DB: f32 = 2.5;\n",
"const TRANSIENT_MAX_GAIN_DB: f32 = 2.5;\nconst TRANSIENT_DETECT_LP_HZ: f32 = 180.0;\n\n// Keep the bigger 10-direction bubble where directional HRTF structure is\n// useful, but do not let ten independent ITDs comb the early-reflection bass.\n// This matches the retained late enclosure's low-frequency coherence boundary.\nconst EARLY_COHERENCE_XOVER_HZ: f32 = 300.0;\n",
'LF constants')

old_struct = '''#[derive(Clone, Copy)]
struct TransientReflectionExciter {
    fast_energy: f32,
    slow_energy: f32,
    envelope: f32,
    fast_alpha: f32,
    slow_alpha: f32,
    release_coeff: f32,
    max_delta: f32,
}
'''
new_struct = '''#[derive(Clone, Copy)]
struct TransientReflectionExciter {
    fast_energy: f32,
    slow_energy: f32,
    envelope: f32,
    fast_alpha: f32,
    slow_alpha: f32,
    release_coeff: f32,
    max_delta: f32,
    low_state_1: f32,
    low_state_2: f32,
    low_alpha: f32,
}
'''
once(old_struct, new_struct, 'transient struct')

old_impl = '''impl TransientReflectionExciter {
    fn new(sample_rate_hz: u32) -> Self {
        let sample_rate_hz = sample_rate_hz.max(1) as f32;
        let one_pole_alpha =
            |time_ms: f32| 1.0 - (-1.0 / (0.001 * time_ms.max(0.01) * sample_rate_hz)).exp();
        Self {
            fast_energy: 0.0,
            slow_energy: 0.0,
            envelope: 0.0,
            fast_alpha: one_pole_alpha(TRANSIENT_FAST_MS),
            slow_alpha: one_pole_alpha(TRANSIENT_SLOW_MS),
            release_coeff: (-1.0 / (0.001 * TRANSIENT_RELEASE_MS.max(0.01) * sample_rate_hz)).exp(),
            max_delta: 10.0_f32.powf(TRANSIENT_MAX_GAIN_DB / 20.0) - 1.0,
        }
    }

    #[inline]
    fn gain(&mut self, input: f32) -> f32 {
        let energy = input * input;
        self.fast_energy += self.fast_alpha * (energy - self.fast_energy);
        self.slow_energy += self.slow_alpha * (energy - self.slow_energy);

        let target = if self.fast_energy > TRANSIENT_MIN_RMS * TRANSIENT_MIN_RMS {
            let positive_rise = (self.fast_energy - self.slow_energy).max(0.0);
            let relative_rise = positive_rise / (self.slow_energy + 1.0e-9);
            ((relative_rise - TRANSIENT_RISE_THRESHOLD)
                / (TRANSIENT_FULL_RISE - TRANSIENT_RISE_THRESHOLD))
                .clamp(0.0, 1.0)
        } else {
            0.0
        };

        if target > self.envelope {
            self.envelope = target;
        } else {
            self.envelope *= self.release_coeff;
        }

        1.0 + self.max_delta * self.envelope
    }
}
'''
new_impl = '''impl TransientReflectionExciter {
    fn new(sample_rate_hz: u32) -> Self {
        let sample_rate_hz = sample_rate_hz.max(1) as f32;
        let one_pole_alpha =
            |time_ms: f32| 1.0 - (-1.0 / (0.001 * time_ms.max(0.01) * sample_rate_hz)).exp();
        Self {
            fast_energy: 0.0,
            slow_energy: 0.0,
            envelope: 0.0,
            fast_alpha: one_pole_alpha(TRANSIENT_FAST_MS),
            slow_alpha: one_pole_alpha(TRANSIENT_SLOW_MS),
            release_coeff: (-1.0 / (0.001 * TRANSIENT_RELEASE_MS.max(0.01) * sample_rate_hz)).exp(),
            max_delta: 10.0_f32.powf(TRANSIENT_MAX_GAIN_DB / 20.0) - 1.0,
            low_state_1: 0.0,
            low_state_2: 0.0,
            low_alpha: 1.0
                - (-std::f32::consts::TAU * TRANSIENT_DETECT_LP_HZ / sample_rate_hz).exp(),
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        // Detect and boost only low-frequency transient content. The old law
        // multiplied the full-band reflection input whenever any lane's onset
        // detector fired, which could make vocals/snares pull the apparent room
        // level around. Two one-pole stages keep bright attacks out of the
        // detector while remaining cheap and causal.
        self.low_state_1 += self.low_alpha * (input - self.low_state_1);
        self.low_state_2 += self.low_alpha * (self.low_state_1 - self.low_state_2);
        let low = self.low_state_2;
        let energy = low * low;
        self.fast_energy += self.fast_alpha * (energy - self.fast_energy);
        self.slow_energy += self.slow_alpha * (energy - self.slow_energy);

        let target = if self.fast_energy > TRANSIENT_MIN_RMS * TRANSIENT_MIN_RMS {
            let positive_rise = (self.fast_energy - self.slow_energy).max(0.0);
            let relative_rise = positive_rise / (self.slow_energy + 1.0e-9);
            ((relative_rise - TRANSIENT_RISE_THRESHOLD)
                / (TRANSIENT_FULL_RISE - TRANSIENT_RISE_THRESHOLD))
                .clamp(0.0, 1.0)
        } else {
            0.0
        };

        if target > self.envelope {
            self.envelope = target;
        } else {
            self.envelope *= self.release_coeff;
        }

        // Exact identity when the transient envelope is closed. When open,
        // only the detector's low-passed component is added, so broadband
        // program level does not breathe with the room excitation.
        input + low * self.max_delta * self.envelope
    }

    #[cfg(test)]
    fn current_gain(&self) -> f32 {
        1.0 + self.max_delta * self.envelope
    }
}
'''
once(old_impl, new_impl, 'transient implementation')

once(
'''        input *= self.transient.gain(input);''',
'''        input = self.transient.process(input);''',
'LF-only transient application')

insert_after = '''impl HrtfDirectionBus {
    fn new(sample_rate_hz: u32, hrir: &HrirSet, direction: [f32; 3]) -> Self {
'''
# We insert the split before HrtfDirectionBus, so use the struct anchor instead.
anchor = '''struct HrtfDirectionBus {
    delay_l: DelayLine,
    delay_r: DelayLine,
    conv_l: EarConvolver,
    conv_r: EarConvolver,
}
'''
replacement = '''struct EarlyCoherenceSplit {
    lp: BiquadCoeffs,
    hp: BiquadCoeffs,
    lp1: BiquadState,
    lp2: BiquadState,
    hp1: BiquadState,
    hp2: BiquadState,
}

impl EarlyCoherenceSplit {
    fn new(sample_rate_hz: u32) -> Self {
        Self {
            lp: butterworth2_lp(EARLY_COHERENCE_XOVER_HZ, sample_rate_hz),
            hp: butterworth2_hp(EARLY_COHERENCE_XOVER_HZ, sample_rate_hz),
            lp1: BiquadState::default(),
            lp2: BiquadState::default(),
            hp1: BiquadState::default(),
            hp2: BiquadState::default(),
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> (f32, f32) {
        // LR4: two cascaded BW2 sections per branch. LP+HP is magnitude-flat
        // with a shared all-pass phase rotation, avoiding a crossover bump/dip.
        let low = biquad(biquad(input, self.lp, &mut self.lp1), self.lp, &mut self.lp2);
        let high = biquad(biquad(input, self.hp, &mut self.hp1), self.hp, &mut self.hp2);
        (low, high)
    }
}

struct HrtfDirectionBus {
    delay_l: DelayLine,
    delay_r: DelayLine,
    conv_l: EarConvolver,
    conv_r: EarConvolver,
}
'''
once(anchor, replacement, 'early coherence split')

once(
'''pub(crate) struct HrtfEarlyReflectionField {
    sources: Vec<Option<SourceReflectionBank>>,
    routes: Vec<Option<[usize; NUM_REFLECTIONS]>>,
    buses: [HrtfDirectionBus; EARLY_HRTF_BUSES],
}
''',
'''pub(crate) struct HrtfEarlyReflectionField {
    sources: Vec<Option<SourceReflectionBank>>,
    routes: Vec<Option<[usize; NUM_REFLECTIONS]>>,
    buses: [HrtfDirectionBus; EARLY_HRTF_BUSES],
    splits: [EarlyCoherenceSplit; EARLY_HRTF_BUSES],
}
''',
'field split storage')

once(
'''        let buses: [HrtfDirectionBus; EARLY_HRTF_BUSES] = std::array::from_fn(|cluster| {
            HrtfDirectionBus::new(sample_rate_hz, &hrir, directions[cluster])
        });

        Self {
            sources,
            routes,
            buses,
        }
''',
'''        let buses: [HrtfDirectionBus; EARLY_HRTF_BUSES] = std::array::from_fn(|cluster| {
            HrtfDirectionBus::new(sample_rate_hz, &hrir, directions[cluster])
        });
        let splits: [EarlyCoherenceSplit; EARLY_HRTF_BUSES] =
            std::array::from_fn(|_| EarlyCoherenceSplit::new(sample_rate_hz));

        Self {
            sources,
            routes,
            buses,
            splits,
        }
''',
'construct coherence splits')

once(
'''            let o = frame * 2;
            for (cluster, bus) in self.buses.iter_mut().enumerate() {
                let (l, r) = bus.process(direction_bus[cluster]);
                out[o] += l;
                out[o + 1] += r;
            }
''',
'''            let o = frame * 2;
            let mut coherent_low = 0.0f32;
            for cluster in 0..EARLY_HRTF_BUSES {
                let (low, high) = self.splits[cluster].process(direction_bus[cluster]);
                coherent_low += low;
                let (l, r) = self.buses[cluster].process(high);
                out[o] += l;
                out[o + 1] += r;
            }
            // Below the crossover, preserve the reflection timing/envelope but
            // collapse directional ITD so the early room cannot comb the bass.
            // sqrt((4/3)/2) is the same total-ear power match used by the HRTF
            // branch, so this is a coherence change rather than a bass boost.
            let low_ear = coherent_low * HRTF_POWER_MATCH;
            out[o] += low_ear;
            out[o + 1] += low_ear;
''',
'coherent low rendering')

# Replace the three old transient tests with LF-specific regression coverage.
start = s.index('    #[test]\n    fn transient_exciter_is_bounded_and_returns_to_unity()')
end = s.index('    #[test]\n    fn center_and_lfe_never_enter_reflection_support()', start)
new_tests = '''    #[test]
    fn transient_exciter_is_low_frequency_only_and_returns_to_identity() {
        let mut exciter = TransientReflectionExciter::new(48_000);
        for _ in 0..1_024 {
            assert_eq!(exciter.process(0.0).to_bits(), 0.0f32.to_bits());
        }

        // A short 70 Hz burst should open the LF transient envelope.
        for sample in 0..2_400 {
            let x = (std::f32::consts::TAU * 70.0 * sample as f32 / 48_000.0).sin() * 0.35;
            let _ = exciter.process(x);
        }
        let peak = exciter.current_gain();
        let maximum = 10.0_f32.powf(TRANSIENT_MAX_GAIN_DB / 20.0);
        assert!(peak > 1.05, "LF burst did not excite early room: {peak}");
        assert!(peak <= maximum + 1.0e-6, "transient gain exceeded bound: {peak}");

        for _ in 0..4_800 {
            let _ = exciter.process(0.0);
        }
        assert!(
            exciter.current_gain() < 1.01,
            "LF transient room gain did not decay: {}",
            exciter.current_gain()
        );
    }

    #[test]
    fn bright_attack_does_not_pump_the_early_room() {
        let mut exciter = TransientReflectionExciter::new(48_000);
        let mut peak = 1.0f32;
        for sample in 0..2_400 {
            let x = (std::f32::consts::TAU * 4_000.0 * sample as f32 / 48_000.0).sin() * 0.35;
            let _ = exciter.process(x);
            peak = peak.max(exciter.current_gain());
        }
        assert!(peak < 1.01, "bright attack pumped early-room level: {peak}");
    }

    #[test]
    fn steady_low_tone_does_not_sustain_transient_excitation() {
        let mut exciter = TransientReflectionExciter::new(48_000);
        let mut max_tail = 1.0f32;
        for sample in 0..48_000 {
            let x = (std::f32::consts::TAU * 80.0 * sample as f32 / 48_000.0).sin() * 0.2;
            let _ = exciter.process(x);
            if sample > 9_600 {
                max_tail = max_tail.max(exciter.current_gain());
            }
        }
        assert!(
            max_tail < 1.005,
            "steady bass kept transient room excitation alive: {max_tail}"
        );
    }

    #[test]
    fn early_coherence_split_sends_bass_low_and_presence_high() {
        fn branch_rms(freq_hz: f32) -> (f32, f32) {
            let mut split = EarlyCoherenceSplit::new(48_000);
            let mut low_energy = 0.0f32;
            let mut high_energy = 0.0f32;
            for sample in 0..48_000 {
                let x = (std::f32::consts::TAU * freq_hz * sample as f32 / 48_000.0).sin();
                let (low, high) = split.process(x);
                if sample >= 24_000 {
                    low_energy += low * low;
                    high_energy += high * high;
                }
            }
            (low_energy.sqrt(), high_energy.sqrt())
        }

        let (bass_low, bass_high) = branch_rms(80.0);
        assert!(bass_low > bass_high * 8.0, "80 Hz leaked into directional branch");
        let (presence_low, presence_high) = branch_rms(2_000.0);
        assert!(presence_high > presence_low * 8.0, "2 kHz leaked into coherent branch");
    }

'''
s = s[:start] + new_tests + s[end:]

# Documentation comments: make the surviving contract explicit.
s = s.replace(
'''//! fast energy envelope with a slow energy envelope. A sharp positive rise may
//! briefly increase only that lane's signal entering the early-reflection delay
//! bank.''',
'''//! fast low-frequency energy envelope with a slow low-frequency envelope. A
//! kick/bass-like positive rise may briefly increase only the low-passed part of
//! that lane entering the early-reflection delay bank.''')

p.write_text(s)
print('patched', p)
