//! Portable authored-scene contract shared by platform adapters and renderers.
//!
//! This crate owns source identity continuity, metric geometry, and sample-time
//! block semantics only. It deliberately contains no Windows, ADM parser,
//! binaural renderer, device, callback, or transport implementation.

pub mod authored_scene;
pub mod rational_time;
pub mod stable_source_slots;
