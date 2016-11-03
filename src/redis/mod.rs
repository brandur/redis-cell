#![allow(dead_code)]

// This should not be public in the long run. Build an abstraction interface
// instead.
pub mod raw;

pub mod store;

use error::ThrottleError;
use libc::{c_int, c_long, c_longlong, size_t};
use std::error::Error;

pub trait Command {
    fn run(&self, r: Redis, args: &[&str]) -> CommandResult;
}

impl Command {}

pub type CommandResult = Result<bool, ThrottleError>;

pub struct Redis {
    ctx: *mut raw::RedisModuleCtx,
}

#[derive(Debug)]
pub enum LogLevel {
    Debug,
    Notice,
    Verbose,
    Warning,
}

#[derive(Debug)]
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
        log_debug!(self, "{} [began] args = {:?}", command, args);

        let terminated_args: Vec<*mut raw::RedisModuleString> = args.iter()
            .map(|a| raw::RedisModule_CreateString(self.ctx, format!("{}\0", a).as_ptr(), a.len()))
            .collect();

        // One would hope that there's a better way to handle a va_list than
        // this, but I can't find it for the life of me.
        //
        // TODO: Note that my main problem turned out to be something else, so
        // it's worth trying to compact this back down to one call again.
        let raw_reply = match args.len() {
            1 => {
                // WARNING: This is downright hazardous, but I've noticed that
                // if I remove this format! from the line of invocation, the
                // right memory layout doesn't make it into Redis (and it will
                // reply with a -1 "unknown" to all calls). This is still
                // unexplained and I need to do more legwork in understanding
                // this.
                raw::call1::RedisModule_Call(self.ctx,
                                             format!("{}\0", command).as_ptr(),
                                             "s\0".as_ptr(),
                                             terminated_args[0])
            }
            2 => {
                raw::call2::RedisModule_Call(self.ctx,
                                             format!("{}\0", command).as_ptr(),
                                             "ss\0".as_ptr(),
                                             terminated_args[0],
                                             terminated_args[1])
            }
            3 => {
                raw::call3::RedisModule_Call(self.ctx,
                                             format!("{}\0", command).as_ptr(),
                                             "sss\0".as_ptr(),
                                             terminated_args[0],
                                             terminated_args[1],
                                             terminated_args[2])
            }
            _ => return Err(error!("Can't support that many CALL arguments")),
        };

        for redis_str in &terminated_args {
            raw::RedisModule_FreeString(self.ctx, *redis_str);
        }

        let reply_res = manifest_redis_reply(raw_reply);
        raw::RedisModule_FreeCallReply(raw_reply);

        match reply_res {
            Ok(ref reply) => {
                log_debug!(self, "{} [ended] result = {:?}", command, reply);
            }
            Err(_) => (),
        }

        reply_res
    }

    /// Coerces a Redis string as an integer.
    ///
    /// Redis is pretty dumb about data types. It nominally supports strings
    /// versus integers, but an integer set in the store will continue to look
    /// like a string (i.e. "1234") until some other operation like INCR forces
    /// it coercion.
    ///
    /// This method coerces a Redis string that looks like an integer into an
    /// integer response. All other types of replies are pass through
    /// unmodified.
    fn coerce_integer(&self,
                      reply_res: Result<Reply, ThrottleError>)
                      -> Result<Reply, ThrottleError> {
        match reply_res {
            Ok(Reply::String(s)) => {
                match s.parse::<i64>() {
                    Ok(n) => Ok(Reply::Integer(n)),
                    _ => Ok(Reply::String(s)),
                }
            }
            _ => reply_res,
        }
    }

    fn expire(&self, key: &str, ttl: i64) -> Result<bool, ThrottleError> {
        let res = try!(self.call("EXPIRE", &[key, ttl.to_string().as_str()]));
        parse_bool(res)
    }

    fn get(&self, key: &str) -> Result<Reply, ThrottleError> {
        self.call("GET", &[key])
    }

    fn log(&self, level: LogLevel, message: &str) {
        raw::RedisModule_Log(self.ctx,
                             format!("{:?}\0", level).to_lowercase().as_ptr(),
                             format!("{}\0", message).as_ptr());
    }

    fn log_debug(&self, message: &str) {
        // TODO: change to actual debug. Notice for now so that we can see
        // things.
        self.log(LogLevel::Notice, message);
    }

    /// Tells Redis that we're about to reply with an (Redis) array.
    ///
    /// Used by invoking once with the expected length and then calling any
    /// combination of the other reply_* methods exactly that number of times.
    ///
    /// The success return value can be safely ignored.
    pub fn reply_array(&self, len: i64) -> Result<bool, ThrottleError> {
        handle_status(raw::RedisModule_ReplyWithArray(self.ctx, len as c_long),
                      "Could not reply with long")
    }

    pub fn reply_integer(&self, integer: i64) -> Result<bool, ThrottleError> {
        handle_status(raw::RedisModule_ReplyWithLongLong(self.ctx, integer as c_longlong),
                      "Could not reply with longlong")
    }

    pub fn reply_string(&self, message: &str) -> Result<bool, ThrottleError> {
        let redis_str = raw::RedisModule_CreateString(self.ctx,
                                                      format!("{}\0", message).as_ptr(),
                                                      message.len());
        let res = handle_status(raw::RedisModule_ReplyWithString(self.ctx, redis_str),
                                "Could not reply with string");
        raw::RedisModule_FreeString(self.ctx, redis_str);
        res
    }

    fn set(&self, key: &str, val: &str) -> Result<bool, ThrottleError> {
        let res = try!(self.call("SET", &[key, val]));
        parse_simple_string(res)
    }

    fn setex(&self, key: &str, ttl: i64, val: &str) -> Result<bool, ThrottleError> {
        let res = try!(self.call("SETEX", &[key, ttl.to_string().as_str(), val]));
        parse_simple_string(res)
    }

    fn setnx(&self, key: &str, val: &str) -> Result<bool, ThrottleError> {
        let res = try!(self.call("SETNX", &[key, val]));
        parse_bool(res)
    }
}

fn handle_status(status: raw::Status, message: &str) -> Result<bool, ThrottleError> {
    match status {
        raw::Status::Ok => Ok(true),
        raw::Status::Err => Err(error!(message)),
    }
}

pub fn harness_command(command: &Command,
                       ctx: *mut raw::RedisModuleCtx,
                       argv: *mut *mut raw::RedisModuleString,
                       argc: c_int)
                       -> raw::Status {
    let r = Redis { ctx: ctx };
    let args = parse_args(argv, argc).unwrap();
    let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    match command.run(r, str_args.as_slice()) {
        Ok(_) => raw::Status::Ok,
        Err(e) => {
            raw::RedisModule_ReplyWithError(ctx,
                                            format!("Throttle error: {}\0", e.description())
                                                .as_ptr());
            raw::Status::Err
        }
    }
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
        raw::ReplyType::Error => Err(error!("Redis replied with an error.")),

        other => Err(error!("Don't yet handle Redis type: {:?}", other)),
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
        // EXPIRE and SETNX are supposed to return a boolean false in their
        // failure case, but this seems to come back as an "unknown" instead so
        // handle that as well.
        Reply::Unknown => Ok(false),
        Reply::Integer(n) if n == 0 => Ok(false),
        Reply::Integer(n) if n == 1 => Ok(true),
        r => Err(error!("Command returned non-boolean value (type was {:?}).", r)),
    }
}

fn parse_simple_string(reply: Reply) -> Result<bool, ThrottleError> {
    match reply {
        // may also return a Redis null, but not with the parameters that
        // we currently allow
        Reply::String(ref s) if s.as_str() == "OK" => Ok(true),
        r => Err(error!("Command returned non-string value (type was {:?}).", r)),
    }
}
