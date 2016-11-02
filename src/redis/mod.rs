#![allow(dead_code)]

// This should not be public in the long run. Build an abstraction interface
// instead.
pub mod raw;

pub mod store;

use error::ThrottleError;
use libc::{c_int, c_longlong, size_t};

pub trait Command {
    fn run(&self, r: Redis, args: &[&str]) -> CommandResult;
}

impl Command {}

pub type CommandResult = Result<bool, ThrottleError>;

pub struct Redis {
    ctx: *mut raw::RedisModuleCtx,
}

pub enum Reply {
    Array,
    Error,
    Integer(i64),
    Nil,
    String(String),
    Unknown,
}

impl Redis {
    fn call(&self, command: &str, args: &[&str]) -> Result<Reply, ThrottleError> {
        let terminated_command = format!("{}\0", command).as_ptr();
        let terminated_args: Vec<*const u8> =
            args.iter().map(|a| format!("{}\0", a).as_ptr()).collect();
        let raw_reply =
            raw::RedisModule_Call(self.ctx, terminated_command, terminated_args.as_slice());
        let reply = manifest_redis_reply(raw_reply);
        raw::RedisModule_FreeCallReply(raw_reply);
        reply
    }

    fn expire(&self, key: &str, ttl: i64) -> Result<bool, ThrottleError> {
        let res = try!(self.call("EXPIRE", &[key, ttl.to_string().as_str()]));
        parse_bool(res)
    }

    fn get(&self, key: &str) -> Result<Reply, ThrottleError> {
        self.call("GET", &[key])
    }

    fn set(&self, key: &str, val: &str) -> Result<String, ThrottleError> {
        let res = try!(self.call("SET", &[key, val]));
        parse_simple_string(res)
    }

    fn setex(&self, key: &str, ttl: i64, val: &str) -> Result<String, ThrottleError> {
        let res = try!(self.call("SET", &[key, val]));
        parse_simple_string(res)
    }

    fn setnx(&self, key: &str, val: &str) -> Result<bool, ThrottleError> {
        let res = try!(self.call("SETNX", &[key, val]));
        parse_bool(res)
    }
}

pub fn harness_command(command: &Command,
                       ctx: *mut raw::RedisModuleCtx,
                       argv: *mut *mut raw::RedisModuleString,
                       argc: c_int)
                       -> CommandResult {
    let r = Redis { ctx: ctx };
    let args = parse_args(argv, argc).unwrap();
    let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    command.run(r, str_args.as_slice())
}

fn manifest_redis_reply(reply: *mut raw::RedisModuleCallReply) -> Result<Reply, ThrottleError> {
    match raw::RedisModule_CallReplyType(reply) {
        raw::ReplyType::Integer => Ok(Reply::Integer(raw::RedisModule_CallReplyInteger(reply))),
        raw::ReplyType::Nil => Ok(Reply::Nil),
        raw::ReplyType::String => {
            let mut length: size_t = 0;
            let bytes = raw::RedisModule_CallReplyStringPtr(reply, &mut length);
            match from_byte_string(bytes, length) {
                Ok(s) => Ok(Reply::String(s)),
                Err(e) => Err(e),
            }
        }
        raw::ReplyType::Unknown => Ok(Reply::Unknown),

        // TODO: I need to actually extract the error from Redis here. Also, it
        // should probably be its own non-generic variety of ThrottleError.
        raw::ReplyType::Error => Err(ThrottleError::generic("Redis replied with an error.")),

        other => {
            Err(ThrottleError::generic(format!("Don't yet handle Redis type: {:?}", other)
                .as_str()))
        }
    }
}

fn manifest_redis_string(redis_str: *mut raw::RedisModuleString) -> Result<String, ThrottleError> {
    let mut length: size_t = 0;
    let bytes = raw::RedisModule_StringPtrLen(redis_str, &mut length);
    from_byte_string(bytes, length)
}

pub fn parse_args(argv: *mut *mut raw::RedisModuleString,
                  argc: c_int)
                  -> Result<Vec<String>, ThrottleError> {
    let mut args: Vec<String> = Vec::with_capacity(argc as usize);
    for i in 0..argc {
        let redis_str = unsafe { *argv.offset(i as isize) };
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

fn parse_bool(reply: Reply) -> Result<bool, ThrottleError> {
    match reply {
        Reply::Integer(n) => {
            match n {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(ThrottleError::generic("Command returned non-boolean value.")),
            }
        }
        _ => Err(ThrottleError::generic("Command returned non-integer value.")),
    }
}

fn parse_simple_string(reply: Reply) -> Result<String, ThrottleError> {
    match reply {
        // may also return a Redis null, but not with the parameters that
        // we currently allow
        Reply::String(s) => {
            match s.as_str() {
                "OK" => Ok(s),
                _ => Err(ThrottleError::generic("Command returned non-simple string value.")),
            }
        }
        _ => Err(ThrottleError::generic("Command returned non-string value.")),
    }
}
