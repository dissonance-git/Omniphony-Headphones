// Compile the production Windows spatial ingress + source-scene lowering inside
// the realtime contract test job. This keeps the lossless object path executable
// without requiring a Windows provider to exist on the CI host.
#[path = "../../windows_host/src/spatial_ingress.rs"]
mod spatial_ingress;
#[path = "../../windows_host/src/spatial_source_frame.rs"]
mod spatial_source_frame;
#[path = "../../windows_host/src/spatial_source_slots.rs"]
mod spatial_source_slots;

use orender_engine::{SourceRendererOptions, SourceSpatialMode, build_source_frame_renderer};
use spatial_ingress::{
    WindowsDynamicObject, WindowsSpatialPosition, WindowsStaticObject, WindowsStaticObjectRole,
    build_windows_spatial_ingress_quantum,
};
use spatial_source_frame::build_windows_spatial_source_frame;

#[test]
fn windows_spatial_quantum_reaches_the_shared_source_renderer_as_authored_objects() {
    const FRAMES: usize = 2_048;
    const SAMPLE_RATE_HZ: u32 = 48_000;

    let front_left = vec![0.20f32; FRAMES];
    let bottom_back_right = vec![-0.12f32; FRAMES];
    let moving_object = (0..FRAMES)
        .map(|index| ((index as f32) * 0.03125).sin() * 0.15)
        .collect::<Vec<_>>();

    let static_objects = [
        WindowsStaticObject {
            role: WindowsStaticObjectRole::FrontLeft,
            windows_position: None,
            mono_pcm: &front_left,
        },
        WindowsStaticObject {
            role: WindowsStaticObjectRole::BottomBackRight,
            windows_position: None,
            mono_pcm: &bottom_back_right,
        },
    ];
    let dynamic_objects = [WindowsDynamicObject {
        stable_id: u64::MAX - 7,
        windows_position: WindowsSpatialPosition::new(0.25, 0.75, -1.25),
        mono_pcm: &moving_object,
    }];

    let ingress = build_windows_spatial_ingress_quantum(&static_objects, &dynamic_objects)
        .expect("lossless Windows ingress quantum");
    let source_frame =
        build_windows_spatial_source_frame(&ingress).expect("renderer-ready authored source frame");

    assert_eq!(source_frame.frame_count, FRAMES);
    assert_eq!(source_frame.source_count(), 3);
    assert!(source_frame.sources[1].authored_position.unwrap()[2] < 0.0);
    assert_eq!(
        source_frame.sources[2].authored_position,
        Some([0.25, 1.25, 0.75])
    );

    let mut renderer = build_source_frame_renderer(
        SAMPLE_RATE_HZ,
        None,
        SourceRendererOptions {
            mode: SourceSpatialMode::FullSphere,
            externalization: false,
            authored_metric_objects: true,
            ..SourceRendererOptions::default()
        },
    )
    .expect("shared Current source renderer");

    let rendered = renderer
        .render_source_frame_with_gain_policy(
            &source_frame.interleaved_pcm,
            &source_frame.sources,
            None,
            0,
            0,
            Vec::new(),
            false,
        )
        .expect("Windows authored objects render through shared source path");

    assert_eq!(rendered.samples.len(), FRAMES * 2);
    assert!(rendered.samples.iter().all(|sample| sample.is_finite()));
    assert!(rendered.samples.iter().any(|sample| sample.abs() > 1.0e-7));
}
