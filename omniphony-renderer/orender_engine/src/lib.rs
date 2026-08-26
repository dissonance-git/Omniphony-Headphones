//! Headless decode→render engine for the `orender` spatial audio renderer.
//!
//! [`Engine`] owns a loaded decoder bridge plugin and a [`SpatialRenderer`], and
//! turns raw compressed packets into VBAP-rendered interleaved multichannel PCM.
//! It performs no audio I/O: the host (the `orender` CLI, or `liborender.so`
//! inside mpv) feeds packets in and consumes rendered samples.

pub mod bridge_loader;
pub mod channel_layout;
pub mod current_authored_bed;
pub mod current_music_support;
pub mod degraded;
pub mod engine;
pub mod events;
#[cfg_attr(test, allow(unused_mut))]
pub mod object_gen;
pub mod osc;
pub mod overlay;
pub mod phantom_extract;
mod phantom_spectral;
pub mod render;
pub mod renderer_build;
pub mod source_renderer_build;
pub mod spatial;
mod stft;
pub mod virtual_bed;

// Current support is renderer-core state, not Windows host state. Keep the
// implementation modules private and expose narrow wrappers above.
mod music_early_reflections;
mod music_late_enclosure;
mod music_support;

pub use channel_layout::label_for_speaker_name;
pub use degraded::{DegradedReporter, start_degraded_reporter};
pub use engine::{Engine, OscOptions, RenderedAudio};
pub use osc::{ObjectMeta, OscSender};
pub use source_renderer_build::{
    SourceRendererOptions, SourceSpatialMode, build_source_frame_renderer,
    source_presentation_policy,
};
/// The shared omniphony config (`~/.config/omniphony/config.yaml`) + its path,
/// re-exported so hosts default to the SAME config as the `orender` CLI + studio
/// (bridge path, layout, OSC settings, render params).
pub use renderer::config::{
    Config, RenderConfig, default_config_path, migrate_legacy_windows_config,
};
pub use virtual_bed::{build_virtual_bed_events, build_virtual_bed_objects};

/// Install the shared live-log logger used by the engine.
///
/// This is the SAME logger the `orender` CLI installs: it writes to stderr **and**
/// buffers records that the OSC listener streams to connected clients (Studio's
/// log panel). Embedding hosts that drive the engine over OSC — notably
/// `orender_ffi` (liborender.so for mpv) — must call this instead of a plain
/// `env_logger`, otherwise `log::*` diagnostics never reach OSC clients.
///
/// `level` is the initial runtime verbosity (OSC-adjustable later via
/// `/omniphony/control/log_level`). Returns `Err` if a global logger is already
/// installed; callers should guard with their own `Once` and ignore that error.
pub fn init_live_logging(level: log::LevelFilter, json: bool) -> Result<(), log::SetLoggerError> {
    sys::live_log::init_logger(level, json)
}
