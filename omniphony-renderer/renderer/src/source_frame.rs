//! Direct renderer path for already-separated causal source channels.
//!
//! This is the path game-music decoders should use once they can provide real
//! source lanes. It deliberately bypasses ordinary stereo scene inference.
//! The protected historical/reference mix remains an external control and must
//! not be included among the object lanes passed here.

use anyhow::{Result, bail};

use crate::source_identity::{SourcePresentationIdentity, source_presentation_identity};
use crate::source_scene::{
    NativeStereoRoute, SourceLaneKind, SourcePresentationPolicy, SourceSceneEvidence,
};
use crate::source_scene_event::present_source_channel;
use crate::spatial_renderer::{ChannelRoute, RenderedFrame, SpatialChannelEvent, SpatialRenderer};

pub struct SourceFrameRenderer {
    renderer: SpatialRenderer,
    policy: SourcePresentationPolicy,
    configured_channels: usize,
    routes: Vec<ChannelRoute>,
    events: Vec<SpatialChannelEvent>,
    scaled_input: Vec<f32>,
    presentation_identities: Vec<Option<SourcePresentationIdentity>>,
    presentation_identity_initialized: Vec<bool>,
}

/// Collapse a historical stereo route to the scalar energy carried by one
/// causal mono source before Omniphony replaces that two-channel projection
/// with a binaural object.
///
/// Signs remain available in `NativeStereoRoute` as polarity/phase evidence;
/// they do not swap sides and therefore enter the energy law squared. The
/// normalization keeps a source routed at unity to both historical outputs at
/// unity, while a unity hard-left/right source carries sqrt(1/2) of that stereo
/// RMS energy.
pub fn route_energy_gain(route: Option<NativeStereoRoute>) -> f32 {
    let Some(route) = route else { return 1.0; };
    if !route.left_gain.is_finite() || !route.right_gain.is_finite() {
        return 0.0;
    }
    ((route.left_gain * route.left_gain + route.right_gain * route.right_gain) * 0.5)
        .sqrt()
        .clamp(0.0, 1.0)
}

/// Renderer-local suppression of already-derived object extent.
///
/// This is deliberately a retention scalar rather than another source-evidence
/// field: 1 keeps the presentation size selected by source/musical policy, 0
/// collapses only the object's extent to a point while leaving its centre,
/// authored route and source identity untouched. Values above 1 are not
/// accepted here because expansion belongs to the ordinary presentation policy.
fn retain_extent(size: [f32; 3], retention: f32) -> [f32; 3] {
    let retention = retention.clamp(0.0, 1.0);
    size.map(|axis| (axis * retention).clamp(0.0, 1.0))
}

impl SourceFrameRenderer {
    pub fn new(renderer: SpatialRenderer, policy: SourcePresentationPolicy) -> Self {
        Self {
            renderer,
            policy,
            configured_channels: 0,
            routes: Vec::new(),
            events: Vec::new(),
            scaled_input: Vec::new(),
            presentation_identities: Vec::new(),
            presentation_identity_initialized: Vec::new(),
        }
    }

    pub fn renderer(&self) -> &SpatialRenderer {
        &self.renderer
    }

    pub fn renderer_mut(&mut self) -> &mut SpatialRenderer {
        &mut self.renderer
    }

    pub fn policy(&self) -> SourcePresentationPolicy {
        self.policy
    }

    pub fn set_policy(&mut self, policy: SourcePresentationPolicy) {
        self.policy = policy;
    }

    pub fn reset_runtime_state(&mut self) {
        self.renderer.reset_runtime_state();
        self.presentation_identities.fill(None);
        self.presentation_identity_initialized.fill(false);
    }

    /// Reset one physical source lane before it is reused for a different
    /// stable identity. Other lanes keep their spatial and filter history.
    pub fn reset_channel_runtime_state(&mut self, channel_idx: usize) {
        self.renderer.reset_channel_runtime_state(channel_idx);
        if let Some(identity) = self.presentation_identities.get_mut(channel_idx) {
            *identity = None;
        }
        if let Some(initialized) = self.presentation_identity_initialized.get_mut(channel_idx) {
            *initialized = false;
        }
    }

    /// Render one block of interleaved already-separated source PCM.
    ///
    /// This compatibility entry point assumes every lane is still pre-route and
    /// therefore derives scalar source energy from `native_stereo_route`.
    pub fn render_source_frame(
        &mut self,
        input_pcm: &[f32],
        sources: &[SourceSceneEvidence],
        sample_pos: u64,
        ramp_length: u32,
        samples_buf: Vec<f32>,
        measure_breakdown: bool,
    ) -> Result<RenderedFrame> {
        self.render_source_frame_with_gain_policy(
            input_pcm,
            sources,
            None,
            sample_pos,
            ramp_length,
            samples_buf,
            measure_breakdown,
        )
    }

    /// Render one block with optional host-owned gain policy.
    ///
    /// `route_gain_preapplied[channel] == true` means the host has already
    /// applied that lane's sample-accurate native gain trajectory to its causal
    /// PCM. Native L/R routing remains available to the scene policy for pose
    /// and polarity evidence, but Omniphony must not scalar-attenuate the PCM a
    /// second time. This is used by SPC where the effective `mChnL/mChnR`
    /// trajectory varies inside the block.
    pub fn render_source_frame_with_gain_policy(
        &mut self,
        input_pcm: &[f32],
        sources: &[SourceSceneEvidence],
        route_gain_preapplied: Option<&[bool]>,
        sample_pos: u64,
        ramp_length: u32,
        samples_buf: Vec<f32>,
        measure_breakdown: bool,
    ) -> Result<RenderedFrame> {
        self.render_source_frame_with_presentation_controls(
            input_pcm,
            sources,
            route_gain_preapplied,
            None,
            None,
            sample_pos,
            ramp_length,
            samples_buf,
            measure_breakdown,
        )
    }

    /// Render one block with host-owned arithmetic policy plus renderer-local
    /// per-lane extent and ramp sidecars.
    ///
    /// `extent_retention[channel]` is a presentation safety control, not source
    /// evidence. `1.0` retains the source's normally derived FullSphere extent;
    /// `0.0` renders the same source centre as a point.
    ///
    /// `presentation_ramp_frames[channel]` optionally overrides only that
    /// source object's geometry/size ramp duration. It exists so independent
    /// attack-body blooms remain time-invariant when unrelated sources insert
    /// exact event boundaries. Source-identity replacement still wins and uses
    /// a zero pose ramp so an unrelated source cannot inherit the old pose.
    pub fn render_source_frame_with_presentation_controls(
        &mut self,
        input_pcm: &[f32],
        sources: &[SourceSceneEvidence],
        route_gain_preapplied: Option<&[bool]>,
        extent_retention: Option<&[f32]>,
        presentation_ramp_frames: Option<&[u32]>,
        sample_pos: u64,
        ramp_length: u32,
        samples_buf: Vec<f32>,
        measure_breakdown: bool,
    ) -> Result<RenderedFrame> {
        let channels = sources.len();
        if let Some(flags) = route_gain_preapplied {
            if flags.len() != channels {
                bail!(
                    "route gain policy width {} does not match {} source channels",
                    flags.len(),
                    channels
                );
            }
        }
        if let Some(retention) = extent_retention {
            if retention.len() != channels {
                bail!(
                    "extent-retention width {} does not match {} source channels",
                    retention.len(),
                    channels
                );
            }
            if let Some((index, value)) = retention
                .iter()
                .copied()
                .enumerate()
                .find(|(_, value)| !value.is_finite() || !(0.0..=1.0).contains(value))
            {
                bail!("extent-retention value at channel {index} is out of range: {value}");
            }
        }
        if let Some(ramps) = presentation_ramp_frames {
            if ramps.len() != channels {
                bail!(
                    "presentation-ramp width {} does not match {} source channels",
                    ramps.len(),
                    channels
                );
            }
        }
        if channels == 0 {
            if !input_pcm.is_empty() {
                bail!("source PCM is non-empty but source channel list is empty");
            }
            return self.renderer.render_frame(
                input_pcm,
                0,
                &[],
                samples_buf,
                measure_breakdown,
            );
        }
        if input_pcm.len() % channels != 0 {
            bail!(
                "source PCM length {} is not divisible by {} source channels",
                input_pcm.len(),
                channels
            );
        }
        if let Some((index, _)) = sources
            .iter()
            .enumerate()
            .find(|(_, source)| source.lane_kind == SourceLaneKind::ReferenceMix)
        {
            bail!(
                "source channel {index} is a protected ReferenceMix; controls must stay outside the object-lane render"
            );
        }

        if self.configured_channels != channels {
            let previous_channels = self.configured_channels;
            self.routes.resize(channels, ChannelRoute::Virtual);
            self.renderer.configure_channel_routing(&self.routes);

            // Source-count changes are ordinary lifecycle events for authored
            // object streams. Keep the surviving prefix continuous and clear
            // only lanes that disappeared or became newly addressable.
            let reset_start = previous_channels.min(channels);
            let reset_end = previous_channels.max(channels);
            for channel_idx in reset_start..reset_end {
                self.renderer.reset_channel_runtime_state(channel_idx);
            }

            self.presentation_identities.resize(channels, None);
            self.presentation_identity_initialized.resize(channels, false);
            self.configured_channels = channels;
        }

        self.events.clear();
        self.events.reserve(channels);
        for (channel_idx, source) in sources.iter().copied().enumerate() {
            let identity = source_presentation_identity(&source);
            let identity_changed = self.presentation_identity_initialized[channel_idx]
                && self.presentation_identities[channel_idx] != identity;

            // A physical lane is not a musical identity. If an unrelated source
            // reuses the same channel, do not interpolate through the outgoing
            // source's old pose. A proven persistent part retains the same
            // identity key and therefore keeps ordinary smooth motion.
            let requested_ramp = presentation_ramp_frames
                .and_then(|ramps| ramps.get(channel_idx))
                .copied()
                .unwrap_or(ramp_length);
            let event_ramp_length = if identity_changed { 0 } else { requested_ramp };
            let mut presented = present_source_channel(
                channel_idx,
                source,
                self.policy,
                Some(sample_pos),
                Some(event_ramp_length),
            );
            if let Some(retention) = extent_retention {
                let size = retain_extent(presented.presentation.size, retention[channel_idx]);
                presented.presentation.size = size;
                if let Some(event) = presented.event.as_mut() {
                    event.size = Some(size);
                }
            }
            let Some(event) = presented.event else {
                bail!("renderable source channel {channel_idx} produced no object event");
            };
            self.events.push(event);
        }

        let gain_for = |channel_idx: usize| {
            if route_gain_preapplied
                .and_then(|flags| flags.get(channel_idx))
                .copied()
                .unwrap_or(false)
            {
                1.0
            } else {
                route_energy_gain(sources[channel_idx].native_stereo_route)
            }
        };

        // Preserve historical source energy unless the host explicitly applied
        // a more precise trajectory already. This stays at float precision
        // rather than quantizing source level into integer-dB object metadata.
        let needs_scaling = (0..channels).any(|channel_idx| {
            (gain_for(channel_idx) - 1.0).abs() > 1.0e-7
        });
        let render_input: &[f32] = if needs_scaling {
            self.scaled_input.resize(input_pcm.len(), 0.0);
            for (frame_in, frame_out) in input_pcm
                .chunks_exact(channels)
                .zip(self.scaled_input.chunks_exact_mut(channels))
            {
                for channel_idx in 0..channels {
                    frame_out[channel_idx] = frame_in[channel_idx] * gain_for(channel_idx);
                }
            }
            &self.scaled_input
        } else {
            input_pcm
        };

        let rendered = self.renderer.render_frame(
            render_input,
            channels,
            &self.events,
            samples_buf,
            measure_breakdown,
        )?;

        // Presentation identity is part of successful renderer history. Do not
        // advance it before render_frame succeeds, or a failed block could make
        // the following call inherit a continuity decision that never sounded.
        for (channel_idx, source) in sources.iter().enumerate() {
            self.presentation_identities[channel_idx] = source_presentation_identity(source);
            self.presentation_identity_initialized[channel_idx] = true;
        }

        Ok(rendered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_identity::{SourcePresentationIdentity, source_presentation_identity};

    #[test]
    fn source_frame_contract_rejects_reference_mix_as_object_lane() {
        let sources = [SourceSceneEvidence {
            lane_kind: SourceLaneKind::ReferenceMix,
            source_id: 1,
            confidence: 1.0,
            ..SourceSceneEvidence::default()
        }];
        assert_eq!(sources[0].lane_kind, SourceLaneKind::ReferenceMix);
    }

    #[test]
    fn source_frame_contract_requires_interleaved_width_to_match_source_count() {
        let sources = [
            SourceSceneEvidence {
                source_id: 1,
                ..SourceSceneEvidence::default()
            },
            SourceSceneEvidence {
                source_id: 2,
                ..SourceSceneEvidence::default()
            },
        ];
        assert_ne!(3usize % sources.len(), 0);
    }

    #[test]
    fn authored_stereo_route_preserves_source_energy_and_not_polarity_as_level() {
        assert_eq!(route_energy_gain(None), 1.0);
        assert!((route_energy_gain(Some(NativeStereoRoute {
            left_gain: 1.0,
            right_gain: 1.0,
        })) - 1.0).abs() < 1.0e-7);
        assert!((route_energy_gain(Some(NativeStereoRoute {
            left_gain: 1.0,
            right_gain: 0.0,
        })) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-7);
        assert!((route_energy_gain(Some(NativeStereoRoute {
            left_gain: -1.0,
            right_gain: 0.5,
        })) - ((1.0_f32 + 0.25) * 0.5).sqrt()).abs() < 1.0e-7);
        assert_eq!(route_energy_gain(Some(NativeStereoRoute {
            left_gain: 0.0,
            right_gain: 0.0,
        })), 0.0);
    }

    #[test]
    fn preapplied_gain_policy_is_width_checked_and_semantically_unity() {
        let sources = [
            SourceSceneEvidence {
                native_stereo_route: Some(NativeStereoRoute { left_gain: 1.0, right_gain: 0.0 }),
                ..SourceSceneEvidence::default()
            },
            SourceSceneEvidence::default(),
        ];
        let flags = [true, false];
        assert_eq!(flags.len(), sources.len());
        assert_eq!(
            if flags[0] { 1.0 } else { route_energy_gain(sources[0].native_stereo_route) },
            1.0
        );
        assert_eq!(route_energy_gain(sources[0].native_stereo_route), std::f32::consts::FRAC_1_SQRT_2);
    }

    #[test]
    fn extent_retention_collapses_only_size() {
        let size = [0.8, 0.6, 0.4];
        assert_eq!(retain_extent(size, 1.0), size);
        assert_eq!(retain_extent(size, 0.0), [0.0; 3]);
        assert_eq!(retain_extent(size, 0.5), [0.4, 0.3, 0.2]);
    }

    #[test]
    fn persistent_part_owns_presentation_continuity_across_source_reuse() {
        let a = SourceSceneEvidence {
            source_id: 10,
            persistent_part_id: Some(77),
            ..SourceSceneEvidence::default()
        };
        let b = SourceSceneEvidence {
            source_id: 11,
            persistent_part_id: Some(77),
            ..SourceSceneEvidence::default()
        };
        let unrelated = SourceSceneEvidence {
            source_id: 12,
            persistent_part_id: None,
            ..SourceSceneEvidence::default()
        };
        assert_eq!(
            source_presentation_identity(&a),
            Some(SourcePresentationIdentity::PersistentPart(77))
        );
        assert_eq!(source_presentation_identity(&a), source_presentation_identity(&b));
        assert_ne!(source_presentation_identity(&b), source_presentation_identity(&unrelated));
    }
}
