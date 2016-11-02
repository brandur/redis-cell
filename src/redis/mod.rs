#![allow(dead_code)]

// This should not be public in the long run. Build an abstraction interface
// instead.
pub mod raw;

pub mod store;

use error::ThrottleError;
use libc::{c_int, c_longlong, size_t};

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
                    _ => Err(ThrottleError::generic("EXPIRE returned non-boolean value.")),
                }
            }
            _ => Err(ThrottleError::generic("EXPIRE returned non-integer value.")),
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
                Err(ThrottleError::generic(format!("Key {:?} is not a type we can handle ({:?}).",
                                                   key,
                                                   reply)
                    .as_str()))
            }
        };
        raw::RedisModule_FreeCallReply(reply);
        ret
    }

    fn set(&self, key: &str, val: &str) -> Result<bool, ThrottleError> {
        let reply = raw::RedisModule_Call(self.ctx,
                                          "SET\0".as_ptr(),
                                          &[format!("{}\0", key).as_ptr(),
                                            format!("{}\0", val).as_ptr()]);
        let reply_ref = unsafe { &mut *reply };
        let res = try!(manifest_redis_reply(reply_ref));
        let ret = match res.as_str() {
            // may also return a Redis null, but not with the parameters that
            // we currently allow
            "OK" => Ok(true),
            _ => Err(ThrottleError::generic("SET returned non-simple string value.")),
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
        let res = try!(manifest_redis_reply(reply_ref));
        let ret = match res.as_str() {
            "OK" => Ok(true),
            _ => Err(ThrottleError::generic("SETEX returned non-simple string value.")),
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
                    _ => Err(ThrottleError::generic("SETNX returned non-boolean value.")),
                }
            }
            _ => Err(ThrottleError::generic("SETNX returned non-integer value.")),
        };
        raw::RedisModule_FreeCallReply(reply);
        ret
    }
}

pub fn harness_command(command: &Command,
                       ctx: *mut raw::RedisModuleCtx,
                       argv: *mut *mut raw::RedisModuleString,
                       argc: c_int)
                       -> CommandResult {
    let r = Redis { ctx: ctx };
    let args = parse_args(argv, argc).unwrap();
    command.run(r, args.iter().map(|s| s.as_str()).collect())
}

pub fn manifest_redis_reply(reply: &mut raw::RedisModuleCallReply)
                            -> Result<String, ThrottleError> {
    match raw::RedisModule_CallReplyType(reply) {
        raw::ReplyType::String => {
            let mut length: size_t = 0;
            let bytes = raw::RedisModule_CallReplyStringPtr(reply, &mut length);
            from_byte_string(bytes, length)
        }
        _ => Err(ThrottleError::generic("Redis reply was not a string.")),
    }
}

pub fn manifest_redis_string(redis_str: &mut raw::RedisModuleString)
                             -> Result<String, ThrottleError> {
    let mut length: size_t = 0;
    let bytes = raw::RedisModule_StringPtrLen(redis_str, &mut length);
    from_byte_string(bytes, length)
}

pub fn parse_args(argv: *mut *mut raw::RedisModuleString,
                  argc: c_int)
                  -> Result<Vec<String>, ThrottleError> {
    let mut args: Vec<String> = Vec::with_capacity(argc as usize);
    for i in 0..argc {
        let redis_str: &mut raw::RedisModuleString = unsafe { &mut *(*argv.offset(i as isize)) };
        args.push(try!(manifest_redis_string(redis_str)));
    }
    Ok(args)
}

fn from_byte_string(byte_str: *const u8, length: size_t) -> Result<String, ThrottleError> {
    let mut vec_str: Vec<u8> = Vec::with_capacity(length as usize);
    for j in 0..length {
        let byte: u8 = unsafe { *byte_str.offset(j as isize) };
        vec_str.insert(j, byte);
    }

    match String::from_utf8(vec_str) {
        Ok(s) => Ok(s),
        Err(e) => Err(ThrottleError::String(e)),
    }
}
