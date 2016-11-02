#![allow(dead_code)]

// This should not be public in the long run. Build an abstraction interface
// instead.
pub mod raw;

pub mod store;

use error::{GenericError, ThrottleError};
use libc;

pub trait Command {
    fn run(&self, r: Redis, args: Vec<&str>) -> CommandResult;
}

impl Command {}

pub type CommandResult = Result<bool, ThrottleError>;

pub struct Redis {
    ctx: *mut raw::RedisModuleCtx,
}

impl Redis {
    fn expire(&self, key: &str, ttl: i64) -> Result<bool, ThrottleError> {
        let reply = raw::RedisModule_Call(self.ctx,
                                          "EXPIRE\0".as_ptr(),
                                          &[format!("{}\0", key).as_ptr(),
                                            format!("{}\0", ttl).as_ptr()]);
        let ret = match raw::RedisModule_CallReplyType(reply) {
            raw::ReplyType::Integer => {
                match raw::RedisModule_CallReplyInteger(reply) {
                    0 => Ok(false),
                    1 => Ok(true),
                    _ => {
                        Err(ThrottleError::Generic(GenericError::new("EXPIRE returned \
                                                                      non-boolean value.")))
                    }
                }
            }
            _ => {
                Err(ThrottleError::Generic(GenericError::new("EXPIRE returned non-integer value.")))
            }
        };
        raw::RedisModule_FreeCallReply(reply);
        ret
    }

    fn get_integer(&self, key: &str) -> Result<i64, ThrottleError> {
        let reply =
            raw::RedisModule_Call(self.ctx, "GET\0".as_ptr(), &[format!("{}\0", key).as_ptr()]);
        let ret = match raw::RedisModule_CallReplyType(reply) {
            raw::ReplyType::Integer => Ok(raw::RedisModule_CallReplyInteger(reply) as i64),
            raw::ReplyType::Null => Ok(-1),
            _ => {
                Err(ThrottleError::Generic(GenericError::new(format!("Key {:?} is not a type \
                                                                      we can handle ({:?}).",
                                                                     key,
                                                                     reply)
                    .as_str())))
            }
        };
        raw::RedisModule_FreeCallReply(reply);
        ret
    }

    fn setex(&self, key: &str, ttl: i64, val: &str) -> Result<bool, ThrottleError> {
        let reply = raw::RedisModule_Call(self.ctx,
                                          "SETEX\0".as_ptr(),
                                          &[format!("{}\0", key).as_ptr(),
                                            format!("{}\0", ttl).as_ptr(),
                                            format!("{}\0", val).as_ptr()]);
        let reply_ref = unsafe { &mut *reply };
        let res = try!(reply_ref.as_string());
        let ret = match res.as_str() {
            "OK" => Ok(true),
            _ => {
                Err(ThrottleError::Generic(GenericError::new("SETEX returned non-simple string \
                                                              value.")))
            }
        };
        raw::RedisModule_FreeCallReply(reply);
        ret
    }

    fn setnx(&self, key: &str, val: &str) -> Result<bool, ThrottleError> {
        let reply = raw::RedisModule_Call(self.ctx,
                                          "SETNX\0".as_ptr(),
                                          &[format!("{}\0", key).as_ptr(),
                                            format!("{}\0", val).as_ptr()]);
        let ret = match raw::RedisModule_CallReplyType(reply) {
            raw::ReplyType::Integer => {
                match raw::RedisModule_CallReplyInteger(reply) {
                    0 => Ok(false),
                    1 => Ok(true),
                    _ => {
                        Err(ThrottleError::Generic(GenericError::new("SETNX returned \
                                                                      non-boolean value.")))
                    }
                }
            }
            _ => {
                Err(ThrottleError::Generic(GenericError::new("SETNX returned non-integer value.")))
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
                  -> Result<Vec<String>, ThrottleError> {
    let mut args: Vec<String> = Vec::with_capacity(argc as usize);
    for i in 0..argc {
        let redis_str: &mut raw::RedisModuleString = unsafe { &mut *(*argv.offset(i as isize)) };
        args.push(try!(redis_str.as_string()));
    }
    Ok(args)
}
