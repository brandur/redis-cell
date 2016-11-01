#![allow(dead_code)]

// This should not be public in the long run. Build an abstraction interface
// instead.
pub mod raw;

pub mod store;

use libc;
use std::string;
use throttle::store::StoreError;

pub type CommandError = StoreError;

pub type CommandResult = Result<bool, CommandError>;

pub trait Command {
    fn run(&self, r: Redis, args: Vec<&str>) -> CommandResult;
}

impl Command {}

pub struct Redis {
    ctx: *mut raw::RedisModuleCtx,
}

impl Redis {
    fn get_integer(&self, key: &str) -> Result<i64, CommandError> {
        let reply =
            raw::RedisModule_Call(self.ctx, "GET\0".as_ptr(), &[format!("{}\0", key).as_ptr()]);
        let ret = match raw::RedisModule_CallReplyType(reply) {
            raw::ReplyType::Integer => Ok(raw::RedisModule_CallReplyInteger(reply) as i64),
            raw::ReplyType::Null => Ok(-1),
            _ => {
                Err(CommandError::new(format!("Key {:?} is not a type we can handle ({:?}).",
                                              key,
                                              reply)
                    .as_str()))
            }
        };
        raw::RedisModule_FreeCallReply(reply);
        ret
    }
}

pub fn harness_command(command: &Command,
                       ctx: *mut raw::RedisModuleCtx,
                       argv: *mut *mut raw::RedisModuleString,
                       argc: libc::c_int)
                       -> CommandResult {
    let r = Redis { ctx: ctx };
    let args = parse_args(argv, argc).unwrap();
    command.run(r, args.iter().map(|s| s.as_str()).collect())
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
