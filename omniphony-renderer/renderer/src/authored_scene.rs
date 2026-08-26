//! Compatibility façade for the portable authored-scene contract.
//!
//! The implementation lives in `scene_contract` so platform adapters can share
//! geometry and sample-time semantics without making the renderer their owner.

pub use scene_contract::authored_scene::*;
