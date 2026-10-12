//! The Taskwarrior engine for the Android app: `tc-core` as a native library, with the replica held in the phone's
//! memory and kept in a file by the app, and synced straight with the R2 bucket over S3 (no Worker in between).
//!
//! `engine` is the engine with no Android in it, tested on any machine; `jni` is the thin layer Kotlin calls.

pub mod engine;
pub mod files;
pub mod filesync;
pub mod jni;
pub mod net;
pub mod s3;
pub mod secrets;
pub mod sigv4;
