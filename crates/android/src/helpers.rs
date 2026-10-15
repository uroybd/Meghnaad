//! What a screen asks of `tc-core` that needs no engine and no lock (`tc_core::helpers`), which the app calls through
//! `NativeEngine`: splitting a filter, writing a date in the user's `dateformat`, and a duration the way Taskwarrior writes it.

pub use tc_core::helpers::{call, format_date, vague, words_json};
