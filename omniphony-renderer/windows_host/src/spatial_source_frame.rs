//! Lossless lowering from Windows Spatial Audio ingress into Omniphony's
//! existing authored-source scene contract.
//!
//! This module is intentionally downstream of `spatial_ingress` and upstream
//! of the allocating source renderer. A future proven Windows provider can copy
//! one OS update quantum into bounded worker-owned storage, build the ingress
//! quantum, then lower it here without flattening static roles or dynamic
//! objects into a speaker mix.
//!
//! Static objects remain fixed-role sources. Dynamic objects retain their full
//! u64 identity, mono PCM and authored Cartesian position. LFE remains real
//! source audio but never becomes a fake HRTF point; it is returned separately
//! for the same coherent post-HRTF treatment used by the native-bed path.

use bridge_api::RChannelLabel;
use renderer::source_scene::{SourceLaneKind, SourceSceneEvidence};

use crate::spatial_ingress::{
    WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4, WindowsSpatialIngressError,
    WindowsSpatialIngressQuantum, WindowsSpatialStaticLane, WindowsStaticObjectRole,
};

const STATIC_SOURCE_ID_BASE: u64 = 0x5354_4154_0000_0000; // "STAT"

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsSpatialSourceKey {
    Static(RChannelLabel),
    Dynamic(u64),
}

/// One renderer-ready source frame plus the non-directional LFE lane.
///
/// `sources` and `interleaved_pcm` are exactly the objects that may enter
/// `SourceFrameRenderer`. `source_keys` preserves the Windows-facing identity
/// beside that generic renderer representation. LFE remains outside the object
/// list by construction.
#[derive(Debug)]
pub struct WindowsSpatialSourceFrame<'a> {
    pub frame_count: usize,
    pub source_keys: Vec<WindowsSpatialSourceKey>,
    pub sources: Vec<SourceSceneEvidence>,
    pub interleaved_pcm: Vec<f32>,
    pub lfe_mono: Option<&'a [f32]>,
}

impl WindowsSpatialSourceFrame<'_> {
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }
}

fn spherical_unit_position(azimuth_deg: f32, elevation_deg: f32) -> [f64; 3] {
    let azimuth = azimuth_deg.to_radians();
    let elevation = elevation_deg.to_radians();
    let horizontal = elevation.cos();
    [
        (azimuth.sin() * horizontal) as f64,
        (azimuth.cos() * horizontal) as f64,
        elevation.sin() as f64,
    ]
}

/// Nominal Omniphony position for a Windows static object role.
///
/// Windows defines static object types as fixed real/virtual speaker locations;
/// unlike dynamic objects, their position is not set by the application. When a
/// provider exposes more exact endpoint geometry, `endpoint_position` wins.
/// Otherwise the role itself supplies the canonical 8.1.4.4 anchor. This is a
/// role-preserving fallback, not inferred scene expansion.
pub fn windows_static_role_position(role: WindowsStaticObjectRole) -> Option<[f64; 3]> {
    let (azimuth_deg, elevation_deg) = match role {
        WindowsStaticObjectRole::FrontLeft => (-30.0, 0.0),
        WindowsStaticObjectRole::FrontRight => (30.0, 0.0),
        WindowsStaticObjectRole::FrontCenter => (0.0, 0.0),
        WindowsStaticObjectRole::LowFrequency => return None,
        WindowsStaticObjectRole::SideLeft => (-90.0, 0.0),
        WindowsStaticObjectRole::SideRight => (90.0, 0.0),
        WindowsStaticObjectRole::BackLeft => (-135.0, 0.0),
        WindowsStaticObjectRole::BackRight => (135.0, 0.0),
        WindowsStaticObjectRole::BackCenter => (180.0, 0.0),
        WindowsStaticObjectRole::TopFrontLeft => (-30.0, 45.0),
        WindowsStaticObjectRole::TopFrontRight => (30.0, 45.0),
        WindowsStaticObjectRole::TopBackLeft => (-135.0, 45.0),
        WindowsStaticObjectRole::TopBackRight => (135.0, 45.0),
        WindowsStaticObjectRole::BottomFrontLeft => (-30.0, -45.0),
        WindowsStaticObjectRole::BottomFrontRight => (30.0, -45.0),
        WindowsStaticObjectRole::BottomBackLeft => (-135.0, -45.0),
        WindowsStaticObjectRole::BottomBackRight => (135.0, -45.0),
    };
    Some(spherical_unit_position(azimuth_deg, elevation_deg))
}

fn static_lane_position(lane: &WindowsSpatialStaticLane<'_>) -> Option<[f64; 3]> {
    lane.endpoint_position
        .or_else(|| windows_static_role_position(lane.role))
}

fn static_source_id(role: WindowsStaticObjectRole) -> u64 {
    STATIC_SOURCE_ID_BASE | role.canonical_scene_index() as u64
}

/// Lower a validated Windows object quantum into the generic authored-source
/// frame consumed by Omniphony's existing source renderer.
///
/// Static lanes are normalized to canonical 8.1.4.4 order, with LFE removed
/// from directional rendering. Dynamic lanes remain in host order so a provider
/// can preserve stable object slots across quanta. No position is quantized to a
/// bed and no missing static role is synthesized as an audible source.
pub fn build_windows_spatial_source_frame<'a>(
    quantum: &WindowsSpatialIngressQuantum<'a>,
) -> Result<WindowsSpatialSourceFrame<'a>, WindowsSpatialIngressError> {
    let frame_count = quantum.frame_count;
    let mut static_slots: [Option<&WindowsSpatialStaticLane<'a>>; 17] = [None; 17];

    for lane in &quantum.static_lanes {
        if lane.mono_pcm.len() != frame_count {
            return Err(WindowsSpatialIngressError::FrameCountMismatch {
                expected: frame_count,
                actual: lane.mono_pcm.len(),
            });
        }
        let index = lane.role.canonical_scene_index();
        if static_slots[index].is_some() {
            return Err(WindowsSpatialIngressError::DuplicateStaticRole(lane.role));
        }
        static_slots[index] = Some(lane);
    }

    for (index, lane) in quantum.dynamic_lanes.iter().enumerate() {
        if lane.mono_pcm.len() != frame_count {
            return Err(WindowsSpatialIngressError::FrameCountMismatch {
                expected: frame_count,
                actual: lane.mono_pcm.len(),
            });
        }
        if quantum.dynamic_lanes[..index]
            .iter()
            .any(|previous| previous.stable_id == lane.stable_id)
        {
            return Err(WindowsSpatialIngressError::DuplicateDynamicId(lane.stable_id));
        }
    }

    let mut directional_static = Vec::with_capacity(16);
    let mut lfe_mono = None;
    for role in WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4 {
        let Some(lane) = static_slots[role.canonical_scene_index()] else {
            continue;
        };
        if role == WindowsStaticObjectRole::LowFrequency {
            lfe_mono = Some(lane.mono_pcm);
        } else {
            directional_static.push(lane);
        }
    }

    let source_count = directional_static.len() + quantum.dynamic_lanes.len();
    let mut source_keys = Vec::with_capacity(source_count);
    let mut sources = Vec::with_capacity(source_count);

    for lane in &directional_static {
        let source_id = static_source_id(lane.role);
        source_keys.push(WindowsSpatialSourceKey::Static(lane.label));
        sources.push(SourceSceneEvidence {
            lane_kind: SourceLaneKind::DrySource,
            source_id,
            persistent_part_id: Some(source_id),
            authored_position: static_lane_position(lane),
            confidence: 1.0,
            ..SourceSceneEvidence::default()
        });
    }

    for lane in &quantum.dynamic_lanes {
        source_keys.push(WindowsSpatialSourceKey::Dynamic(lane.stable_id));
        sources.push(SourceSceneEvidence {
            lane_kind: SourceLaneKind::DrySource,
            source_id: lane.stable_id,
            persistent_part_id: Some(lane.stable_id),
            authored_position: Some(lane.authored_position),
            confidence: 1.0,
            ..SourceSceneEvidence::default()
        });
    }

    let mut interleaved_pcm = Vec::with_capacity(frame_count.saturating_mul(source_count));
    for frame_index in 0..frame_count {
        for lane in &directional_static {
            interleaved_pcm.push(lane.mono_pcm[frame_index]);
        }
        for lane in &quantum.dynamic_lanes {
            interleaved_pcm.push(lane.mono_pcm[frame_index]);
        }
    }

    Ok(WindowsSpatialSourceFrame {
        frame_count,
        source_keys,
        sources,
        interleaved_pcm,
        lfe_mono,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatial_ingress::{
        WindowsDynamicObject, WindowsSpatialPosition, WindowsStaticObject,
        build_windows_spatial_ingress_quantum,
    };

    fn approx_eq(left: f64, right: f64) {
        assert!((left - right).abs() < 1.0e-6, "{left} != {right}");
    }

    #[test]
    fn canonical_static_roles_retain_real_upper_and_lower_hemispheres() {
        let upper = windows_static_role_position(WindowsStaticObjectRole::TopFrontLeft).unwrap();
        let lower = windows_static_role_position(WindowsStaticObjectRole::BottomFrontLeft).unwrap();
        assert!(upper[2] > 0.0);
        assert!(lower[2] < 0.0);
        assert_eq!(
            windows_static_role_position(WindowsStaticObjectRole::LowFrequency),
            None
        );
    }

    #[test]
    fn source_frame_preserves_roles_dynamic_identity_pcm_and_lfe_separately() {
        let front = [0.10, 0.20];
        let lower_back = [0.30, 0.40];
        let lfe = [0.50, 0.60];
        let dynamic = [0.70, 0.80];
        let static_objects = [
            WindowsStaticObject {
                role: WindowsStaticObjectRole::BottomBackRight,
                windows_position: None,
                mono_pcm: &lower_back,
            },
            WindowsStaticObject {
                role: WindowsStaticObjectRole::LowFrequency,
                windows_position: Some(WindowsSpatialPosition::new(9.0, 9.0, 9.0)),
                mono_pcm: &lfe,
            },
            WindowsStaticObject {
                role: WindowsStaticObjectRole::FrontLeft,
                windows_position: None,
                mono_pcm: &front,
            },
        ];
        let dynamic_objects = [WindowsDynamicObject {
            stable_id: u64::MAX,
            windows_position: WindowsSpatialPosition::new(0.25, -0.5, -1.5),
            mono_pcm: &dynamic,
        }];

        let ingress = build_windows_spatial_ingress_quantum(&static_objects, &dynamic_objects).unwrap();
        let frame = build_windows_spatial_source_frame(&ingress).unwrap();

        assert_eq!(frame.frame_count, 2);
        assert_eq!(frame.source_count(), 3);
        assert_eq!(
            frame.source_keys,
            vec![
                WindowsSpatialSourceKey::Static(RChannelLabel::L),
                WindowsSpatialSourceKey::Static(RChannelLabel::Bbr),
                WindowsSpatialSourceKey::Dynamic(u64::MAX),
            ]
        );
        assert_eq!(frame.lfe_mono, Some(lfe.as_slice()));
        assert_eq!(
            frame.interleaved_pcm,
            vec![0.10, 0.30, 0.70, 0.20, 0.40, 0.80]
        );
        assert_eq!(frame.sources[2].source_id, u64::MAX);
        assert_eq!(frame.sources[2].persistent_part_id, Some(u64::MAX));
        assert_eq!(frame.sources[2].authored_position, Some([0.25, 1.5, -0.5]));
        assert_eq!(frame.sources[2].confidence, 1.0);
    }

    #[test]
    fn static_role_geometry_is_used_when_provider_has_no_coordinate_payload() {
        let pcm = [1.0];
        let ingress = build_windows_spatial_ingress_quantum(
            &[WindowsStaticObject {
                role: WindowsStaticObjectRole::FrontLeft,
                windows_position: None,
                mono_pcm: &pcm,
            }],
            &[],
        )
        .unwrap();
        let frame = build_windows_spatial_source_frame(&ingress).unwrap();
        let position = frame.sources[0].authored_position.unwrap();
        approx_eq(position[0], -0.5);
        approx_eq(position[1], 0.866_025_4);
        approx_eq(position[2], 0.0);
    }

    #[test]
    fn provider_geometry_overrides_nominal_static_role_anchor_without_axis_loss() {
        let pcm = [1.0];
        let ingress = build_windows_spatial_ingress_quantum(
            &[WindowsStaticObject {
                role: WindowsStaticObjectRole::BottomBackRight,
                windows_position: Some(WindowsSpatialPosition::new(0.25, -0.75, 1.5)),
                mono_pcm: &pcm,
            }],
            &[],
        )
        .unwrap();
        let frame = build_windows_spatial_source_frame(&ingress).unwrap();
        assert_eq!(frame.sources[0].authored_position, Some([0.25, -1.5, -0.75]));
    }

    #[test]
    fn partial_static_sets_do_not_fabricate_missing_audible_sources() {
        let pcm = [0.25];
        let ingress = build_windows_spatial_ingress_quantum(
            &[WindowsStaticObject {
                role: WindowsStaticObjectRole::BottomBackRight,
                windows_position: None,
                mono_pcm: &pcm,
            }],
            &[],
        )
        .unwrap();
        let frame = build_windows_spatial_source_frame(&ingress).unwrap();
        assert_eq!(frame.source_count(), 1);
        assert_eq!(
            frame.source_keys,
            vec![WindowsSpatialSourceKey::Static(RChannelLabel::Bbr)]
        );
        assert_eq!(frame.interleaved_pcm, vec![0.25]);
        assert!(frame.sources[0].authored_position.unwrap()[2] < 0.0);
    }

    #[test]
    fn lfe_never_enters_directional_source_frame() {
        let lfe = [1.0, -1.0];
        let ingress = build_windows_spatial_ingress_quantum(
            &[WindowsStaticObject {
                role: WindowsStaticObjectRole::LowFrequency,
                windows_position: None,
                mono_pcm: &lfe,
            }],
            &[],
        )
        .unwrap();
        let frame = build_windows_spatial_source_frame(&ingress).unwrap();
        assert_eq!(frame.source_count(), 0);
        assert!(frame.source_keys.is_empty());
        assert!(frame.interleaved_pcm.is_empty());
        assert_eq!(frame.lfe_mono, Some(lfe.as_slice()));
    }

    #[test]
    fn source_frame_revalidates_width_before_renderer_consumption() {
        let pcm = [0.25];
        let quantum = WindowsSpatialIngressQuantum {
            frame_count: 2,
            static_lanes: vec![WindowsSpatialStaticLane {
                role: WindowsStaticObjectRole::FrontCenter,
                label: RChannelLabel::C,
                endpoint_position: None,
                mono_pcm: &pcm,
            }],
            dynamic_lanes: Vec::new(),
        };
        assert_eq!(
            build_windows_spatial_source_frame(&quantum).unwrap_err(),
            WindowsSpatialIngressError::FrameCountMismatch {
                expected: 2,
                actual: 1,
            }
        );
    }
}
