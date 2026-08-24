use omniphony_realtime::{
    OmniphonyRealtimeConfig, omniphony_realtime_create, omniphony_realtime_destroy,
    omniphony_realtime_process_f32, omniphony_realtime_processed_blocks,
    omniphony_realtime_rendered_frames, omniphony_realtime_set_mode,
};
use std::thread;
use std::time::{Duration, Instant};

const MODE_IDENTITY: u32 = 0;
const MODE_CURRENT: u32 = 1;
const BLOCK_FRAMES: usize = 960;
const OUTPUT_CEILING: f32 = 0.891_250_9;
const RELEASE_WORKER_DEADLINE_SECONDS: u64 = 20;

fn config() -> OmniphonyRealtimeConfig {
    OmniphonyRealtimeConfig {
        sample_rate_hz: 48_000,
        channels: 2,
    }
}

unsafe fn drive_until_rendered_block(
    processor: *mut omniphony_realtime::OmniphonyRealtimeProcessor,
    input: &[f32],
    output: &mut [f32],
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(RELEASE_WORKER_DEADLINE_SECONDS);
    while Instant::now() < deadline {
        output.fill(f32::NAN);
        assert_eq!(
            unsafe {
                omniphony_realtime_process_f32(
                    processor,
                    input.as_ptr(),
                    output.as_mut_ptr(),
                    BLOCK_FRAMES,
                )
            },
            0
        );
        assert!(output.iter().all(|sample| sample.is_finite()));
        if unsafe { omniphony_realtime_processed_blocks(processor) } > 0
            && unsafe { omniphony_realtime_rendered_frames(processor) } > 0
            && output.iter().any(|sample| sample.abs() > 1.0e-6)
        {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "full Current DSP liveness is an optimized-build contract"
)]
fn current_worker_round_trips_audio_and_can_be_recreated_in_one_process() {
    let config = config();

    unsafe {
        let processor = omniphony_realtime_create(&config);
        assert!(!processor.is_null());

        assert_eq!(omniphony_realtime_set_mode(processor, MODE_CURRENT), 0);

        // Exactly one Current worker block (20 ms at 48 kHz). Use a quiet but
        // nonzero stereo signal so the test can distinguish real worker output
        // from the startup-silence fallback without turning this into a sound
        // tuning assertion.
        let mut input = vec![0.0f32; BLOCK_FRAMES * 2];
        for frame in 0..BLOCK_FRAMES {
            input[frame * 2] = 0.05;
            input[frame * 2 + 1] = -0.025;
        }
        let mut output = vec![f32::NAN; input.len()];
        assert!(
            drive_until_rendered_block(processor, &input, &mut output),
            "Current worker did not complete and return rendered PCM before the release deadline"
        );

        // This is the lifecycle the eventual tray control will exercise. It
        // specifically guards against one-shot global bridge registration.
        assert_eq!(omniphony_realtime_set_mode(processor, MODE_IDENTITY), 0);
        assert_eq!(omniphony_realtime_set_mode(processor, MODE_CURRENT), 0);
        assert_eq!(omniphony_realtime_set_mode(processor, MODE_IDENTITY), 0);

        omniphony_realtime_destroy(processor);
    }
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "full Current DSP liveness is an optimized-build contract"
)]
fn current_worker_never_exceeds_the_protected_master_ceiling() {
    let config = config();

    unsafe {
        let processor = omniphony_realtime_create(&config);
        assert!(!processor.is_null());
        assert_eq!(omniphony_realtime_set_mode(processor, MODE_CURRENT), 0);

        // Deliberately exceed full scale. This is not a listening fixture. It is
        // an adversarial transport invariant proving that the retained Current
        // master guard still owns the native worker path.
        let mut hot = vec![0.0f32; BLOCK_FRAMES * 2];
        for frame in 0..BLOCK_FRAMES {
            hot[frame * 2] = if frame % 2 == 0 { 2.0 } else { -2.0 };
            hot[frame * 2 + 1] = if frame % 3 == 0 { -1.75 } else { 1.75 };
        }
        let mut output = vec![f32::NAN; hot.len()];
        assert!(
            drive_until_rendered_block(processor, &hot, &mut output),
            "hot Current block did not cross the output ring before the release deadline"
        );

        let peak = output
            .iter()
            .fold(0.0f32, |maximum, sample| maximum.max(sample.abs()));
        assert!(
            peak <= OUTPUT_CEILING + 1.0e-6,
            "native Current peak {peak} exceeded protected ceiling {OUTPUT_CEILING}"
        );

        omniphony_realtime_destroy(processor);
    }
}
