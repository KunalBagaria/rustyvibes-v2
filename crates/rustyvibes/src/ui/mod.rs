//! The menu bar user interface (AppKit through objc2).

mod about;
mod delegate;
pub mod icons;
mod login;
pub mod menu;
pub mod onboarding;
pub mod views;

pub use delegate::run;
