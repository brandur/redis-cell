#![allow(dead_code)]

// This should not be public in the long run. Build an abstraction interface
// instead.
pub mod ffi;

pub mod store;

pub trait Command {
    fn name(&self) -> &'static str;
    fn run(&self, r: Redis, args: Vec<&str>);
}

pub struct Redis {
}

impl Redis {}

fn get(key: &str) {
    // ffi::RedisModule_Call(
}
