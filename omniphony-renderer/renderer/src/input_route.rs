//! Input-routing law for Omniphony's headphone renderer.
//!
//! This module deliberately contains no DSP. It encodes the architectural
//! invariant that every source has one authoritative ingress and receives at
//! most one spatialization pass.
//!
//! The router must never infer spatial structure from material that already
//! carries authored geometry, and it must never HRTF-render material that is
//! already binaural. Ordinary stereo is the only representation that enters
//! the stereo-evidence path.

/// Representation presented to Omniphony by the host/decoder.
///
/// This is metadata/contract state, not something the renderer should guess
/// from PCM samples. In particular, a two-channel binaural master is not safely
/// distinguishable from ordinary stereo by signal inspection alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputRepresentation {
    /// Finished two-channel master with no authoritative spatial metadata.
    Stereo,
    /// Discrete authored speaker/channel geometry, e.g. 5.1/7.1/7.1.4.
    AuthoredMultichannel,
    /// Authored object/scene geometry supplied by the media pipeline.
    AuthoredObjects,
    /// Causal/source-native lanes recovered by a trusted upstream source model.
    RecoveredSources,
    /// Final two-channel binaural signal that has already been spatialized.
    Binaural,
}

/// Mutually exclusive render routes.
///
/// Keeping this as an enum instead of a bag of booleans makes illegal
/// combinations unrepresentable: there is no route that can simultaneously
/// run stereo inference and claim authored geometry, and no binaural route can
/// accidentally invoke the HRTF renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderRoute {
    /// Preserve the finished stereo master and derive only justified residual
    /// spatial support before one binaural render of that support.
    StereoCurrent,
    /// Render discrete authored channels directly from their supplied geometry.
    AuthoredSpatial,
    /// Render trusted recovered sources through source-aware presentation.
    RecoveredSourceScene,
    /// Preserve an already-binaural master. Headphone/output correction may be
    /// applied later, but no spatial inference or HRTF/room render belongs here.
    BinauralPassthrough,
}

impl RenderRoute {
    /// Ordinary stereo is the only route allowed to run stereo inference.
    #[inline]
    pub const fn runs_stereo_inference(self) -> bool {
        matches!(self, Self::StereoCurrent)
    }

    /// Whether Omniphony's acoustic scene -> ears renderer runs for this route.
    #[inline]
    pub const fn runs_acoustic_renderer(self) -> bool {
        !matches!(self, Self::BinauralPassthrough)
    }

    /// Whether the original two-channel program remains an authoritative direct
    /// path rather than being replaced by inferred spatial support.
    #[inline]
    pub const fn preserves_input_master(self) -> bool {
        matches!(self, Self::StereoCurrent | Self::BinauralPassthrough)
    }

    /// Whether position/channel geometry is authored upstream and therefore
    /// must pass through without stereo reinterpretation.
    #[inline]
    pub const fn uses_authored_geometry(self) -> bool {
        matches!(self, Self::AuthoredSpatial)
    }

    /// Number of Omniphony spatialization passes permitted by this route.
    ///
    /// Stereo, authored spatial, and recovered-source scenes each enter exactly
    /// one acoustic render. Binaural material has already completed that job.
    #[inline]
    pub const fn spatialization_passes(self) -> u8 {
        if self.runs_acoustic_renderer() { 1 } else { 0 }
    }
}

/// Resolve host/decoder representation to one renderer route.
///
/// Multichannel and object media intentionally converge on the same authored
/// spatial path. Their transport representation may differ; their authority
/// law does not.
#[inline]
pub const fn route_for_input(input: InputRepresentation) -> RenderRoute {
    match input {
        InputRepresentation::Stereo => RenderRoute::StereoCurrent,
        InputRepresentation::AuthoredMultichannel | InputRepresentation::AuthoredObjects => {
            RenderRoute::AuthoredSpatial
        }
        InputRepresentation::RecoveredSources => RenderRoute::RecoveredSourceScene,
        InputRepresentation::Binaural => RenderRoute::BinauralPassthrough,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_stereo_is_the_only_inference_route() {
        let stereo = route_for_input(InputRepresentation::Stereo);
        assert_eq!(stereo, RenderRoute::StereoCurrent);
        assert!(stereo.runs_stereo_inference());
        assert!(stereo.runs_acoustic_renderer());
        assert!(stereo.preserves_input_master());
        assert_eq!(stereo.spatialization_passes(), 1);

        for input in [
            InputRepresentation::AuthoredMultichannel,
            InputRepresentation::AuthoredObjects,
            InputRepresentation::RecoveredSources,
            InputRepresentation::Binaural,
        ] {
            assert!(!route_for_input(input).runs_stereo_inference());
        }
    }

    #[test]
    fn authored_spatial_media_keeps_geometry_and_gets_one_render() {
        for input in [
            InputRepresentation::AuthoredMultichannel,
            InputRepresentation::AuthoredObjects,
        ] {
            let route = route_for_input(input);
            assert_eq!(route, RenderRoute::AuthoredSpatial);
            assert!(route.uses_authored_geometry());
            assert!(route.runs_acoustic_renderer());
            assert!(!route.preserves_input_master());
            assert_eq!(route.spatialization_passes(), 1);
        }
    }

    #[test]
    fn recovered_sources_bypass_stereo_inference_but_still_render_once() {
        let route = route_for_input(InputRepresentation::RecoveredSources);
        assert_eq!(route, RenderRoute::RecoveredSourceScene);
        assert!(!route.runs_stereo_inference());
        assert!(route.runs_acoustic_renderer());
        assert_eq!(route.spatialization_passes(), 1);
    }

    #[test]
    fn binaural_input_cannot_be_spatialized_twice() {
        let route = route_for_input(InputRepresentation::Binaural);
        assert_eq!(route, RenderRoute::BinauralPassthrough);
        assert!(!route.runs_stereo_inference());
        assert!(!route.runs_acoustic_renderer());
        assert!(route.preserves_input_master());
        assert_eq!(route.spatialization_passes(), 0);
    }

    #[test]
    fn every_input_has_at_most_one_omniphony_spatialization_pass() {
        for input in [
            InputRepresentation::Stereo,
            InputRepresentation::AuthoredMultichannel,
            InputRepresentation::AuthoredObjects,
            InputRepresentation::RecoveredSources,
            InputRepresentation::Binaural,
        ] {
            assert!(route_for_input(input).spatialization_passes() <= 1);
        }
    }
}
