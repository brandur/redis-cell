#![allow(dead_code)]

// This should not be public in the long run. Build an abstraction interface
// instead.
pub mod ffi;

pub mod store;

fn get(key: &str) {}
