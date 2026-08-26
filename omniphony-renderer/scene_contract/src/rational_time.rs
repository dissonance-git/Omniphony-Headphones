//! Exact rational source-time lowering into integer sample spans.
//!
//! Metadata formats such as ADM may describe block timing at fractional sample
//! boundaries. The scene contract keeps that conversion deterministic and
//! callback-independent without depending on any particular metadata parser.
//!
//! Integer sample selection follows the established half-open processing rule:
//! a sample `s` belongs to `[start, end)` when `start <= s < end`. Therefore
//! fractional boundaries lower to integer sample indices with `ceil(start)` and
//! `ceil(end)`.

use crate::authored_scene::SampleSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RationalSeconds {
    pub numerator: u64,
    pub denominator: u64,
}

impl RationalSeconds {
    pub fn new(numerator: u64, denominator: u64) -> Result<Self, SourceTimeError> {
        if denominator == 0 {
            return Err(SourceTimeError::ZeroDenominator);
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceTimeError {
    ZeroDenominator,
    ZeroSampleRate,
    ArithmeticOverflow,
    SpanTooLong,
}

pub fn ceil_sample_index(
    time: RationalSeconds,
    sample_rate_hz: u32,
) -> Result<u64, SourceTimeError> {
    if sample_rate_hz == 0 {
        return Err(SourceTimeError::ZeroSampleRate);
    }
    let scaled = (time.numerator as u128)
        .checked_mul(sample_rate_hz as u128)
        .ok_or(SourceTimeError::ArithmeticOverflow)?;
    let denominator = time.denominator as u128;
    let rounded = ceil_div(scaled, denominator)?;
    u64::try_from(rounded).map_err(|_| SourceTimeError::ArithmeticOverflow)
}

pub fn sample_span_from_rational_times(
    start: RationalSeconds,
    duration: RationalSeconds,
    sample_rate_hz: u32,
) -> Result<SampleSpan, SourceTimeError> {
    if sample_rate_hz == 0 {
        return Err(SourceTimeError::ZeroSampleRate);
    }

    let start_num = start.numerator as u128;
    let start_den = start.denominator as u128;
    let duration_num = duration.numerator as u128;
    let duration_den = duration.denominator as u128;

    let end_num = start_num
        .checked_mul(duration_den)
        .and_then(|value| {
            duration_num
                .checked_mul(start_den)
                .and_then(|duration_scaled| value.checked_add(duration_scaled))
        })
        .ok_or(SourceTimeError::ArithmeticOverflow)?;
    let end_den = start_den
        .checked_mul(duration_den)
        .ok_or(SourceTimeError::ArithmeticOverflow)?;

    let start_sample = ceil_sample_index(start, sample_rate_hz)?;
    let end_scaled = end_num
        .checked_mul(sample_rate_hz as u128)
        .ok_or(SourceTimeError::ArithmeticOverflow)?;
    let end_sample_u128 = ceil_div(end_scaled, end_den)?;
    let end_sample = u64::try_from(end_sample_u128)
        .map_err(|_| SourceTimeError::ArithmeticOverflow)?;

    let frame_count = end_sample
        .checked_sub(start_sample)
        .ok_or(SourceTimeError::ArithmeticOverflow)?;
    let frame_count = u32::try_from(frame_count).map_err(|_| SourceTimeError::SpanTooLong)?;

    Ok(SampleSpan::new(start_sample, frame_count))
}

fn ceil_div(numerator: u128, denominator: u128) -> Result<u128, SourceTimeError> {
    if denominator == 0 {
        return Err(SourceTimeError::ZeroDenominator);
    }
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    Ok(quotient + u128::from(remainder != 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_sample_boundaries_remain_exact() {
        let time = RationalSeconds::new(1, 2).unwrap();
        assert_eq!(ceil_sample_index(time, 48_000).unwrap(), 24_000);
    }

    #[test]
    fn fractional_sample_boundaries_round_up() {
        let half_sample = RationalSeconds::new(1, 96_000).unwrap();
        assert_eq!(ceil_sample_index(half_sample, 48_000).unwrap(), 1);
    }

    #[test]
    fn half_open_fractional_block_matches_reference_sample_selection() {
        // 0.5 samples <= s < 1.5 samples affects integer sample 1 only.
        let start = RationalSeconds::new(1, 96_000).unwrap();
        let duration = RationalSeconds::new(1, 48_000).unwrap();
        assert_eq!(
            sample_span_from_rational_times(start, duration, 48_000).unwrap(),
            SampleSpan::new(1, 1)
        );
    }

    #[test]
    fn adjacent_fractional_blocks_do_not_overlap_or_leave_a_gap() {
        let first = sample_span_from_rational_times(
            RationalSeconds::new(1, 96_000).unwrap(),
            RationalSeconds::new(1, 48_000).unwrap(),
            48_000,
        )
        .unwrap();
        let second = sample_span_from_rational_times(
            RationalSeconds::new(3, 96_000).unwrap(),
            RationalSeconds::new(1, 48_000).unwrap(),
            48_000,
        )
        .unwrap();
        assert_eq!(first, SampleSpan::new(1, 1));
        assert_eq!(second, SampleSpan::new(2, 1));
        assert_eq!(first.end_sample_exclusive(), second.start_sample);
    }

    #[test]
    fn invalid_time_domain_is_rejected() {
        assert_eq!(
            RationalSeconds::new(1, 0),
            Err(SourceTimeError::ZeroDenominator)
        );
        let time = RationalSeconds::new(0, 1).unwrap();
        assert_eq!(
            ceil_sample_index(time, 0),
            Err(SourceTimeError::ZeroSampleRate)
        );
    }
}
