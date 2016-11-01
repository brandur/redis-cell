#![allow(dead_code)]

// This should not be public in the long run. Build an abstraction interface
// instead.
pub mod raw;

pub mod store;

use libc;

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
    let mut args: Vec<String> = Vec::with_capacity(argc as usize);
    let r = Redis {};
    for i in 0..argc {
        let redis_str: *mut raw::RedisModuleString = unsafe { *argv.offset(i as isize) };
        let mut length: libc::size_t = 0;
        let byte_str = raw::RedisModule_StringPtrLen(redis_str, &mut length);

        let mut vec_str: Vec<u8> = Vec::with_capacity(length as usize);
        for j in 0..length {
            let byte: u8 = unsafe { *byte_str.offset(j as isize) };
            vec_str[j] = byte;
        }

        let rust_str = String::from_utf8(vec_str).unwrap();
        args.push(rust_str);
    }
}
