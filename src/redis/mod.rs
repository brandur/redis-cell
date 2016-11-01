#![allow(dead_code)]

// This should not be public in the long run. Build an abstraction interface
// instead.
pub mod ffi;

pub mod store;

pub trait Command {
    fn run(&self, r: Redis, args: Vec<&str>);
}

impl Command {}

pub struct Redis {
}

impl Redis {}

fn get(key: &str) {
    // ffi::RedisModule_Call(
}
