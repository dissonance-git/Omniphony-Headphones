//! Semantic boundary between a future Windows Spatial Audio host and Omniphony.
//!
//! This module deliberately does not open `ISpatialAudioClient` or claim that a
//! normal APO can intercept another application's object stream. It defines the
//! lossless data contract that a proven Windows host boundary must satisfy:
//! preserve the 17 static 8.1.4.4 roles, preserve dynamic object identity and
//! PCM, and convert Windows listener-relative coordinates into Omniphony's
//! listener-relative coordinates without quantizing dynamic objects to a bed.
//!
//! Windows Spatial Audio uses listener-relative Cartesian coordinates with
//! +X right, +Y up, and +Z behind the listener. Omniphony's source scene uses
//! +X right, +Y forward, and +Z up, so the lossless axis conversion is
//! `[x, y, z] -> [x, -z, y]`.

use renderer::authored_scene::{MetricPosition, radial_distance_m};

/// The 17 static spatial roles in the canonical Windows 8.1.4.4 bed.
///
/// Discriminants intentionally match Omniphony's canonical scene order:
/// `L R C LFE Ls Rs Lb Rb Cb Tfl Tfr Tbl Tbr Bfl Bfr Bbl Bbr`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WindowsStaticObjectRole {
    FrontLeft = 0,
    FrontRight = 1,
    FrontCenter = 2,
    LowFrequency = 3,
    SideLeft = 4,
    SideRight = 5,
    BackLeft = 6,
    BackRight = 7,
    BackCenter = 8,
    TopFrontLeft = 9,
    TopFrontRight = 10,
    TopBackLeft = 11,
    TopBackRight = 12,
    BottomFrontLeft = 13,
    BottomFrontRight = 14,
    BottomBackLeft = 15,
    BottomBackRight = 16,
}

pub const WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4: [WindowsStaticObjectRole; 17] = [
    WindowsStaticObjectRole::FrontLeft,
    WindowsStaticObjectRole::FrontRight,
    WindowsStaticObjectRole::FrontCenter,
    WindowsStaticObjectRole::LowFrequency,
    WindowsStaticObjectRole::SideLeft,
    WindowsStaticObjectRole::SideRight,
    WindowsStaticObjectRole::BackLeft,
    WindowsStaticObjectRole::BackRight,
    WindowsStaticObjectRole::BackCenter,
    WindowsStaticObjectRole::TopFrontLeft,
    WindowsStaticObjectRole::TopFrontRight,
    WindowsStaticObjectRole::TopBackLeft,
    WindowsStaticObjectRole::TopBackRight,
    WindowsStaticObjectRole::BottomFrontLeft,
    WindowsStaticObjectRole::BottomFrontRight,
    WindowsStaticObjectRole::BottomBackLeft,
    WindowsStaticObjectRole::BottomBackRight,
];

impl WindowsStaticObjectRole {
    pub const fn canonical_scene_index(self) -> usize {
        self as usize
    }

    pub const fn from_canonical_scene_index(index: u32) -> Option<Self> {
        Some(match index {
            0 => Self::FrontLeft,
            1 => Self::FrontRight,
            2 => Self::FrontCenter,
            3 => Self::LowFrequency,
            4 => Self::SideLeft,
            5 => Self::SideRight,
            6 => Self::BackLeft,
            7 => Self::BackRight,
            8 => Self::BackCenter,
            9 => Self::TopFrontLeft,
            10 => Self::TopFrontRight,
            11 => Self::TopBackLeft,
            12 => Self::TopBackRight,
            13 => Self::BottomFrontLeft,
            14 => Self::BottomFrontRight,
            15 => Self::BottomBackLeft,
            16 => Self::BottomBackRight,
            _ => return None,
        })
    }

    pub const fn is_directional(self) -> bool {
        !matches!(self, Self::LowFrequency)
    }

    pub const fn is_upper(self) -> bool {
        matches!(
            self,
            Self::TopFrontLeft | Self::TopFrontRight | Self::TopBackLeft | Self::TopBackRight
        )
    }

    pub const fn is_lower(self) -> bool {
        matches!(
            self,
            Self::BottomFrontLeft
                | Self::BottomFrontRight
                | Self::BottomBackLeft
                | Self::BottomBackRight
        )
    }
}

/// Listener-relative position supplied by Windows Spatial Audio, in meters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowsSpatialPosition {
    pub x_right_m: f32,
    pub y_up_m: f32,
    pub z_back_m: f32,
}

impl WindowsSpatialPosition {
    pub const fn new(x_right_m: f32, y_up_m: f32, z_back_m: f32) -> Self {
        Self {
            x_right_m,
            y_up_m,
            z_back_m,
        }
    }

    /// Convert Windows axes to the portable metric source-scene axes.
    ///
    /// Omniphony: +X right, +Y forward, +Z up.
    pub fn to_omniphony_metric_xyz(self) -> MetricPosition {
        [
            self.x_right_m as f64,
            -self.z_back_m as f64,
            self.y_up_m as f64,
        ]
    }

    pub fn radial_distance_m(self) -> f64 {
        radial_distance_m(self.to_omniphony_metric_xyz())
    }
}

/// A static Windows Spatial Audio object for one update quantum.
///
/// `windows_position` should come from the active spatial endpoint's static
/// object geometry when the host API exposes it. Omniphony does not invent a
/// replacement position here. LFE is explicitly non-directional.
#[derive(Clone, Copy, Debug)]
pub struct WindowsStaticObject<'a> {
    pub role: WindowsStaticObjectRole,
    pub windows_position: Option<WindowsSpatialPosition>,
    pub mono_pcm: &'a [f32],
}

impl<'a> WindowsStaticObject<'a> {
    pub fn omniphony_metric_position(&self) -> Option<MetricPosition> {
        if !self.role.is_directional() {
            return None;
        }
        self.windows_position
            .map(WindowsSpatialPosition::to_omniphony_metric_xyz)
    }
}

/// A dynamic Windows Spatial Audio object for one update quantum.
///
/// The stable ID belongs to the Windows-facing host adapter. Reusing it across
/// update quanta is what lets Omniphony preserve source identity while the
/// supplied position moves continuously in 3-D space.
#[derive(Clone, Copy, Debug)]
pub struct WindowsDynamicObject<'a> {
    pub stable_id: u64,
    pub windows_position: WindowsSpatialPosition,
    pub mono_pcm: &'a [f32],
}

impl<'a> WindowsDynamicObject<'a> {
    pub fn omniphony_metric_position(&self) -> MetricPosition {
        self.windows_position.to_omniphony_metric_xyz()
    }

    pub fn radial_distance_m(&self) -> f64 {
        self.windows_position.radial_distance_m()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_roles_cover_canonical_8_1_4_4_once() {
        assert_eq!(WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4.len(), 17);
        for (index, role) in WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4.into_iter().enumerate() {
            assert_eq!(role.canonical_scene_index(), index);
            assert_eq!(
                WindowsStaticObjectRole::from_canonical_scene_index(index as u32),
                Some(role)
            );
        }
        assert_eq!(WindowsStaticObjectRole::from_canonical_scene_index(17), None);
        assert_eq!(
            WindowsStaticObjectRole::from_canonical_scene_index(u32::MAX),
            None
        );
    }

    #[test]
    fn lower_and_upper_hemispheres_remain_distinct() {
        let lower = WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4
            .into_iter()
            .filter(|role| role.is_lower())
            .count();
        let upper = WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4
            .into_iter()
            .filter(|role| role.is_upper())
            .count();
        assert_eq!(lower, 4);
        assert_eq!(upper, 4);
    }

    #[test]
    fn windows_axes_convert_without_bed_quantization() {
        assert_eq!(
            WindowsSpatialPosition::new(1.0, 0.0, 0.0).to_omniphony_metric_xyz(),
            [1.0, 0.0, 0.0]
        );
        assert_eq!(
            WindowsSpatialPosition::new(0.0, 1.0, 0.0).to_omniphony_metric_xyz(),
            [0.0, 0.0, 1.0]
        );
        assert_eq!(
            WindowsSpatialPosition::new(0.0, 0.0, -1.0).to_omniphony_metric_xyz(),
            [0.0, 1.0, 0.0]
        );
        assert_eq!(
            WindowsSpatialPosition::new(0.0, 0.0, 1.0).to_omniphony_metric_xyz(),
            [0.0, -1.0, 0.0]
        );
    }

    #[test]
    fn windows_metric_geometry_preserves_radial_distance() {
        let position = WindowsSpatialPosition::new(3.0, 0.0, -4.0);
        assert_eq!(position.to_omniphony_metric_xyz(), [3.0, 4.0, 0.0]);
        assert_eq!(position.radial_distance_m(), 5.0);
    }

    #[test]
    fn dynamic_object_keeps_identity_while_position_moves() {
        let pcm = [0.25, -0.25];
        let first = WindowsDynamicObject {
            stable_id: 41,
            windows_position: WindowsSpatialPosition::new(-0.75, 0.2, -1.5),
            mono_pcm: &pcm,
        };
        let second = WindowsDynamicObject {
            stable_id: 41,
            windows_position: WindowsSpatialPosition::new(0.9, -0.1, 0.4),
            mono_pcm: &pcm,
        };
        assert_eq!(first.stable_id, second.stable_id);
        assert_eq!(first.mono_pcm, second.mono_pcm);
        assert_ne!(first.omniphony_metric_position(), second.omniphony_metric_position());
        assert_eq!(first.omniphony_metric_position(), [-0.75, 1.5, 0.2]);
        assert_eq!(second.omniphony_metric_position(), [0.9, -0.4, -0.1]);
    }

    #[test]
    fn lfe_never_becomes_a_fake_point_source() {
        let pcm = [1.0];
        let object = WindowsStaticObject {
            role: WindowsStaticObjectRole::LowFrequency,
            windows_position: Some(WindowsSpatialPosition::new(9.0, 9.0, 9.0)),
            mono_pcm: &pcm,
        };
        assert_eq!(object.omniphony_metric_position(), None);
    }

    #[test]
    fn static_object_uses_supplied_endpoint_geometry() {
        let pcm = [0.5];
        let object = WindowsStaticObject {
            role: WindowsStaticObjectRole::BottomBackRight,
            windows_position: Some(WindowsSpatialPosition::new(0.6, -0.7, 0.8)),
            mono_pcm: &pcm,
        };
        let position = object.omniphony_metric_position().expect("directional static position");
        let expected = [0.6_f64, -0.8, -0.7];
        for (actual, expected) in position.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
        }
    }
}
