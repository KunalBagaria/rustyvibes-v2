//! The `.rvpack` soundpack format shared by the Rustyvibes app (reader) and its
//! build tooling (writer).

pub mod format;

pub use format::{
    Clip, ClipRef, FormatError, GUARD_AFTER, GUARD_BEFORE, KEY_SLOTS, Meta, PackData, PackView,
};
