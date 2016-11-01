#![allow(dead_code)]

// This should not be public in the long run. Build an abstraction interface
// instead.
pub mod raw;

pub mod store;

use libc;
use std::string;

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
    let args = parse_args(argv, argc).unwrap();
    command.run(r, args.iter().map(|s| s.as_str()).collect());
}

pub fn parse_args(argv: *mut *mut raw::RedisModuleString,
                  argc: libc::c_int)
                  -> Result<Vec<String>, string::FromUtf8Error> {
    let mut args: Vec<String> = Vec::with_capacity(argc as usize);
    for i in 0..argc {
        let redis_str: &mut raw::RedisModuleString = unsafe { &mut *(*argv.offset(i as isize)) };
        args.push(try!(redis_str.as_string()));
    }
    Ok(args)
}
