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
    ctx: *mut raw::RedisModuleCtx,
}

impl Redis {}

fn get(key: &str) {
    // raw::RedisModule_Call(
}

pub fn harness_command(command: &Command,
                       ctx: *mut raw::RedisModuleCtx,
                       argv: *mut *mut raw::RedisModuleString,
                       argc: libc::c_int) {
    let r = Redis { ctx: ctx };
    let args = parse_args(argv, argc);
    command.run(r, args.iter().map(|s| s.as_str()).collect());
}

pub fn parse_args(argv: *mut *mut raw::RedisModuleString, argc: libc::c_int) -> Vec<String> {
    let mut args: Vec<String> = Vec::with_capacity(argc as usize);
    for i in 0..argc {
        let redis_str: &mut raw::RedisModuleString = unsafe { &mut *(*argv.offset(i as isize)) };
        args.push(redis_str.as_string().unwrap());
    }
    args
}
