use omniphony_source::{
    OmniphonySourceConfig, OmniphonySourceEvidenceV1, SOURCE_HRIR_SYNTHETIC, SOURCE_LANE_DRY,
    SOURCE_SPATIAL_FULL_SPHERE, SOURCE_SPATIAL_NATIVE_ROUTING, omniphony_source_create,
    omniphony_source_destroy, omniphony_source_process_f32, omniphony_source_reset,
    omniphony_source_set_spatial_mode,
};

const SAMPLE_RATE: u32 = 48_000;
const FRAMES: usize = 2_048;

fn test_signal() -> Vec<f32> {
    (0..FRAMES)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            0.06 * (std::f32::consts::TAU * 683.0 * t).sin()
                + 0.025 * (std::f32::consts::TAU * 1_877.0 * t).sin()
        })
        .collect()
}

fn render(
    processor: *mut omniphony_source::OmniphonySourceProcessor,
    input: &[f32],
    evidence: &OmniphonySourceEvidenceV1,
) -> Vec<f32> {
    let mut output = vec![0.0f32; FRAMES * 2];
    let status = unsafe {
        omniphony_source_process_f32(
            processor,
            input.as_ptr(),
            evidence,
            1,
            FRAMES,
            0,
            0,
            output.as_mut_ptr(),
        )
    };
    assert_eq!(status, 0);
    assert!(output.iter().all(|sample| sample.is_finite()));
    output
}

#[test]
fn native_to_fullsphere_switch_changes_presentation_without_recreating_processor() {
    let config = OmniphonySourceConfig {
        sample_rate_hz: SAMPLE_RATE,
        spatial_mode: SOURCE_SPATIAL_NATIVE_ROUTING,
        externalization: 0,
        hrir_source: SOURCE_HRIR_SYNTHETIC,
        unit_scale_m: 1.0,
        reflection_level: 0.0,
    };
    let processor = unsafe { omniphony_source_create(&config) };
    assert!(!processor.is_null());

    let evidence = OmniphonySourceEvidenceV1 {
        lane_kind: SOURCE_LANE_DRY,
        source_id: 0xA11D_10,
        confidence: 1.0,
        ..OmniphonySourceEvidenceV1::default()
    };
    let input = test_signal();

    // Materialize the renderer's lazy channel/cascade state once, then put every
    // compared render behind the same public stream-reset boundary. Comparing
    // first-ever lazy startup with a reset renderer tests construction, not the
    // NativeRouting -> FullSphere -> NativeRouting round trip owned here.
    let _warmup = render(processor, &input, &evidence);
    assert_eq!(unsafe { omniphony_source_reset(processor) }, 0);
    let native = render(processor, &input, &evidence);

    assert_eq!(unsafe { omniphony_source_reset(processor) }, 0);
    let native_after_plain_reset = render(processor, &input, &evidence);
    let plain_reset_delta = (native
        .iter()
        .zip(&native_after_plain_reset)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f32>()
        / native.len() as f32)
        .sqrt();
    assert!(
        plain_reset_delta < 1.0e-6,
        "a plain reset must recover NativeRouting before testing any mode change; delta_rms={plain_reset_delta}"
    );

    assert_eq!(
        unsafe { omniphony_source_set_spatial_mode(processor, SOURCE_SPATIAL_FULL_SPHERE) },
        0
    );
    // Reset the causal render history so this comparison isolates the requested
    // presentation mode rather than inheriting convolution/ramp state from the
    // NativeRouting render. The processor and its topology remain the same.
    assert_eq!(unsafe { omniphony_source_reset(processor) }, 0);
    let full_sphere = render(processor, &input, &evidence);

    let delta_rms = (native
        .iter()
        .zip(&full_sphere)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f32>()
        / native.len() as f32)
        .sqrt();
    assert!(
        delta_rms > 1.0e-5,
        "runtime mode switch must audibly open FullSphere; delta_rms={delta_rms}"
    );

    assert_eq!(
        unsafe { omniphony_source_set_spatial_mode(processor, SOURCE_SPATIAL_NATIVE_ROUTING) },
        0
    );
    assert_eq!(unsafe { omniphony_source_reset(processor) }, 0);
    let native_again = render(processor, &input, &evidence);
    let roundtrip_delta = (native
        .iter()
        .zip(&native_again)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f32>()
        / native.len() as f32)
        .sqrt();
    assert!(
        roundtrip_delta < 1.0e-6,
        "switching back must recover NativeRouting deterministically; delta_rms={roundtrip_delta}"
    );

    unsafe { omniphony_source_destroy(processor) };
}
