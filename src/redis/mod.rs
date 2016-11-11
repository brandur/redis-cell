#![allow(dead_code)]

// This should not be public in the long run. Build an abstraction interface
// instead.
pub mod raw;

use error::CellError;
use libc::{c_int, c_long, c_longlong, size_t};
use std::error::Error;
use std::iter;

/// Command is a basic trait for a new command to be registered with a Redis
/// module.
pub trait Command {
    // Should return the name of the command to be registered.
    fn name(&self) -> &'static str;

    // Run the command.
    fn run(&self, r: Redis, args: &[&str]) -> Result<(), CellError>;

    // Should return any flags to be registered with the name as a string
    // separated list. See the Redis module API documentation for a complete
    // list of the ones that are available.
    fn str_flags(&self) -> &'static str;
}

impl Command {
    /// Provides a basic wrapper for a command's implementation that parses
    /// arguments to Rust data types and handles the OK/ERR reply back to Redis.
    pub fn harness(command: &Command,
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
                raw::reply_with_error(ctx,
                                      format!("Cell error: {}\0", e.description())
                                          .as_ptr());
                raw::Status::Err
            }
        }
    }
}

/// LogLevel is a level of logging to be specified with a Redis log directive.
#[derive(Debug)]
pub enum LogLevel {
    Debug,
    Notice,
    Verbose,
    Warning,
}

/// Redis is a structure that's designed to give us a high-level interface to
/// the Redis module API by abstracting away the raw C FFI calls.
pub struct Redis {
    ctx: *mut raw::RedisModuleCtx,
}

impl Redis {
    pub fn call(&self, command: &str, args: &[&str]) -> Result<Reply, CellError> {
        log_debug!(self, "{} [began] args = {:?}", command, args);

        // We use a "format" string to tell redis what types we're passing in.
        // Currently we just pass everything as a string so this is just the
        // character "s" repeated as many times as we have arguments.
        //
        // It would be nice to start passing some parameters as their actual
        // type (for example, i64s as long longs), but Redis stringifies these
        // on the other end anyway so the practical benefit will be minimal.
        let format: String = iter::repeat("s").take(args.len()).collect();

        let terminated_args: Vec<*mut raw::RedisModuleString> = args.iter()
            .map(|a| raw::create_string(self.ctx, format!("{}\0", a).as_ptr(), a.len()))
            .collect();

        // One would hope that there's a better way to handle a va_list than
        // this, but I can't find it for the life of me.
        let raw_reply = match args.len() {
            1 => {
                // WARNING: This is downright hazardous, but I've noticed that
                // if I remove this format! from the line of invocation, the
                // right memory layout doesn't make it into Redis (and it will
                // reply with a -1 "unknown" to all calls). This is still
                // unexplained and I need to do more legwork in understanding
                // this.
                //
                // Still, this works fine and will continue to work as long as
                // it's left unchanged.
                raw::call1::call(self.ctx,
                                 format!("{}\0", command).as_ptr(),
                                 format!("{}\0", format).as_ptr(),
                                 terminated_args[0])
            }
            2 => {
                raw::call2::call(self.ctx,
                                 format!("{}\0", command).as_ptr(),
                                 format!("{}\0", format).as_ptr(),
                                 terminated_args[0],
                                 terminated_args[1])
            }
            3 => {
                raw::call3::call(self.ctx,
                                 format!("{}\0", command).as_ptr(),
                                 format!("{}\0", format).as_ptr(),
                                 terminated_args[0],
                                 terminated_args[1],
                                 terminated_args[2])
            }
            _ => return Err(error!("Can't support that many CALL arguments")),
        };

        for redis_str in &terminated_args {
            raw::free_string(self.ctx, *redis_str);
        }

        let reply_res = manifest_redis_reply(raw_reply);
        raw::free_call_reply(raw_reply);

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
    /// its coercion.
    ///
    /// This method coerces a Redis string that looks like an integer into an
    /// integer response. All other types of replies are pass through
    /// unmodified.
    pub fn coerce_integer(&self,
                          reply_res: Result<Reply, CellError>)
                          -> Result<Reply, CellError> {
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

    pub fn expire(&self, key: &str, ttl: i64) -> Result<bool, CellError> {
        let res = self.call("EXPIRE", &[key, ttl.to_string().as_str()])?;
        parse_bool(res)
    }

    pub fn get(&self, key: &str) -> Result<Reply, CellError> {
        self.call("GET", &[key])
    }

    pub fn log(&self, level: LogLevel, message: &str) {
        raw::log(self.ctx,
                 format!("{:?}\0", level).to_lowercase().as_ptr(),
                 format!("{}\0", message).as_ptr());
    }

    pub fn log_debug(&self, message: &str) {
        // TODO: change to actual debug. Notice for now so that we can see
        // things.
        self.log(LogLevel::Notice, message);
    }

    /// Tells Redis that we're about to reply with an (Redis) array.
    ///
    /// Used by invoking once with the expected length and then calling any
    /// combination of the other reply_* methods exactly that number of times.
    pub fn reply_array(&self, len: i64) -> Result<(), CellError> {
        handle_status(raw::reply_with_array(self.ctx, len as c_long),
                      "Could not reply with long")
    }

    pub fn reply_integer(&self, integer: i64) -> Result<(), CellError> {
        handle_status(raw::reply_with_long_long(self.ctx, integer as c_longlong),
                      "Could not reply with longlong")
    }

    pub fn reply_string(&self, message: &str) -> Result<(), CellError> {
        let redis_str = raw::create_string(self.ctx,
                                           format!("{}\0", message).as_ptr(),
                                           message.len());
        let res = handle_status(raw::reply_with_string(self.ctx, redis_str),
                                "Could not reply with string");
        raw::free_string(self.ctx, redis_str);
        res
    }

    pub fn set(&self, key: &str, val: &str) -> Result<(), CellError> {
        let res = self.call("SET", &[key, val])?;
        parse_simple_string(res)
    }

    pub fn setex(&self, key: &str, ttl: i64, val: &str) -> Result<(), CellError> {
        let res = self.call("SETEX", &[key, ttl.to_string().as_str(), val])?;
        parse_simple_string(res)
    }

    pub fn setnx(&self, key: &str, val: &str) -> Result<bool, CellError> {
        let res = self.call("SETNX", &[key, val])?;
        parse_bool(res)
    }
}

/// Reply represents the various types of a replies that we can receive after
/// executing a Redis command.
#[derive(Debug)]
pub enum Reply {
    Array,
    Error,
    Integer(i64),
    Nil,
    String(String),
    Unknown,
}

fn handle_status(status: raw::Status, message: &str) -> Result<(), CellError> {
    match status {
        raw::Status::Ok => Ok(()),
        raw::Status::Err => Err(error!(message)),
    }
}

fn manifest_redis_reply(reply: *mut raw::RedisModuleCallReply)
                        -> Result<Reply, CellError> {
    match raw::call_reply_type(reply) {
        raw::ReplyType::Integer => Ok(Reply::Integer(raw::call_reply_integer(reply))),
        raw::ReplyType::Nil => Ok(Reply::Nil),
        raw::ReplyType::String => {
            let mut length: size_t = 0;
            let bytes = raw::call_reply_string_ptr(reply, &mut length);
            from_byte_string(bytes, length).map(|s| Reply::String(s))
        }
        raw::ReplyType::Unknown => Ok(Reply::Unknown),

        // TODO: I need to actually extract the error from Redis here. Also, it
        // should probably be its own non-generic variety of CellError.
        raw::ReplyType::Error => Err(error!("Redis replied with an error.")),

        other => Err(error!("Don't yet handle Redis type: {:?}", other)),
    }
}

fn manifest_redis_string(redis_str: *mut raw::RedisModuleString)
                         -> Result<String, CellError> {
    let mut length: size_t = 0;
    let bytes = raw::string_ptr_len(redis_str, &mut length);
    from_byte_string(bytes, length)
}

fn parse_args(argv: *mut *mut raw::RedisModuleString,
              argc: c_int)
              -> Result<Vec<String>, CellError> {
    let mut args: Vec<String> = Vec::with_capacity(argc as usize);
    for i in 0..argc {
        let redis_str = unsafe { *argv.offset(i as isize) };
        args.push(manifest_redis_string(redis_str)?);
    }
    Ok(args)
}

fn from_byte_string(byte_str: *const u8, length: size_t) -> Result<String, CellError> {
    let mut vec_str: Vec<u8> = Vec::with_capacity(length as usize);
    for j in 0..length {
        let byte: u8 = unsafe { *byte_str.offset(j as isize) };
        vec_str.insert(j, byte);
    }

    match String::from_utf8(vec_str) {
        Ok(s) => Ok(s),
        Err(e) => Err(CellError::String(e)),
    }
}

fn parse_bool(reply: Reply) -> Result<bool, CellError> {
    match reply {
        Reply::Integer(n) if n == 0 => Ok(false),
        Reply::Integer(n) if n == 1 => Ok(true),
        r => Err(error!("Command returned non-boolean value (type was {:?}).", r)),
    }
}

fn parse_simple_string(reply: Reply) -> Result<(), CellError> {
    match reply {
        // may also return a Redis null, but not with the parameters that
        // we currently allow
        Reply::String(ref s) if s.as_str() == "OK" => Ok(()),
        r => Err(error!("Command returned non-string value (type was {:?}).", r)),
    }
}
