//! Node.js bindings for Revet via NAPI-RS
//!
//! This crate provides JavaScript/TypeScript bindings for the Revet core library.
//! It will be used to distribute Revet via npm.

use napi_derive::napi;

#[napi]
pub fn analyze_repository(path: String) -> String {
    // TODO: Implement Node.js API
    // This is a placeholder for now
    format!("Would analyze repository at: {}", path)
}

#[napi]
pub fn get_version() -> String {
    revet_core::VERSION.to_string()
}
