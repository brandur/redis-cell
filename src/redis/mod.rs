#![allow(dead_code)]

// This should not be public in the long run. Build an abstraction interface
// instead.
pub mod raw;

pub mod store;

use libc;
use std::ffi;

pub trait Command {
    fn run(&self, r: Redis, args: Vec<&str>);
}

impl Command {}

pub struct Redis {
}

impl Redis {}

fn get(key: &str) {
    // raw::RedisModule_Call(
}

pub fn harness_command(command: &Command,
                       ctx: *mut raw::RedisModuleCtx,
                       argv: *mut *mut raw::RedisModuleString,
                       argc: libc::c_int) {
    let mut args: Vec<&str> = Vec::with_capacity(argc as usize);
    let r = Redis {};
    for i in 0..argc {
        let redisStr: *mut raw::RedisModuleString = unsafe { *argv.offset(i as isize) };
        let mut length: libc::size_t = 0;
        let byteStr = raw::RedisModule_StringPtrLen(redisStr, &mut length);

        let mut vecStr: Vec<u8> = Vec::with_capacity(length as usize);
        for j in 0..length {
            let byte: u8 = unsafe { *byteStr.offset(j as isize) };
            vecStr[j] = byte;
        }

        let rustStr = String::from_utf8(vecStr).unwrap();
        args.push(rustStr.as_str());
    }
}
