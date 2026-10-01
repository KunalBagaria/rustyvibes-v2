//! The `.rvpack` soundpack format shared by the Rustyvibes app (reader) and its
//! build tooling (writer), plus macOS keyboard geometry.

pub mod format;
pub mod keys;
pub mod mechvibes;

pub use format::{
    Clip, ClipRef, FormatError, GUARD_AFTER, GUARD_BEFORE, KEY_SLOTS, Meta, PackData, PackView,
};
