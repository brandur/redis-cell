// `raw` should not be public in the long run. Build an abstraction interface
// instead.
//
// We have to disable a couple Clippy checks here because we'll otherwise have
// warnings thrown from within macros provided by the `bigflags` package.
#[cfg_attr(feature = "cargo-clippy",
           allow(redundant_field_names, suspicious_arithmetic_impl))]
pub mod raw;

use error::CellError;
use libc::{c_int, c_long, c_longlong, size_t};
use std::error::Error;
use std::iter;
use std::ptr;
use std::string;
use time;

/// `LogLevel` is a level of logging to be specified with a Redis log directive.
#[derive(Clone, Copy, Debug)]
pub enum LogLevel {
    Debug,
    Notice,
    Verbose,
    Warning,
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
    pub fn harness(
        command: &Command,
        ctx: *mut raw::RedisModuleCtx,
        argv: *mut *mut raw::RedisModuleString,
        argc: c_int,
    ) -> raw::Status {
        let r = Redis { ctx };
        let args = parse_args(argv, argc).unwrap();
        let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        match command.run(r, str_args.as_slice()) {
            Ok(_) => raw::Status::Ok,
            Err(e) => {
                raw::reply_with_error(
                    ctx,
                    format!("Cell error: {}\0", e.description()).as_ptr(),
                );
                raw::Status::Err
            }
        }
    }
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

        let terminated_args: Vec<RedisString> =
            args.iter().map(|s| self.create_string(s)).collect();

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
                raw::call1::call(
                    self.ctx,
                    format!("{}\0", command).as_ptr(),
                    format!("{}\0", format).as_ptr(),
                    terminated_args[0].str_inner,
                )
            }
            2 => raw::call2::call(
                self.ctx,
                format!("{}\0", command).as_ptr(),
                format!("{}\0", format).as_ptr(),
                terminated_args[0].str_inner,
                terminated_args[1].str_inner,
            ),
            3 => raw::call3::call(
                self.ctx,
                format!("{}\0", command).as_ptr(),
                format!("{}\0", format).as_ptr(),
                terminated_args[0].str_inner,
                terminated_args[1].str_inner,
                terminated_args[2].str_inner,
            ),
            _ => return Err(error!("Can't support that many CALL arguments")),
        };

        let reply_res = manifest_redis_reply(raw_reply);
        raw::free_call_reply(raw_reply);

        if let Ok(ref reply) = reply_res {
            log_debug!(self, "{} [ended] result = {:?}", command, reply);
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
    /// integer response. All other types of replies are passed through
    /// unmodified.
    pub fn coerce_integer(
        &self,
        reply_res: Result<Reply, CellError>,
    ) -> Result<Reply, CellError> {
        match reply_res {
            Ok(Reply::String(s)) => match s.parse::<i64>() {
                Ok(n) => Ok(Reply::Integer(n)),
                _ => Ok(Reply::String(s)),
            },
            _ => reply_res,
        }
    }

    pub fn create_string(&self, s: &str) -> RedisString {
        RedisString::create(self.ctx, s)
    }

    pub fn log(&self, level: LogLevel, message: &str) {
        raw::log(
            self.ctx,
            format!("{:?}\0", level).to_lowercase().as_ptr(),
            format!("{}\0", message).as_ptr(),
        );
    }

    pub fn log_debug(&self, message: &str) {
        // Note that we log our debug messages as notice level in Redis. This
        // is so that they'll show up with default configuration. Our debug
        // logging will get compiled out in a release build so this won't
        // result in undue noise in production.
        self.log(LogLevel::Notice, message);
    }

    /// Opens a Redis key for read access.
    pub fn open_key(&self, key: &str) -> RedisKey {
        RedisKey::open(self.ctx, key)
    }

    /// Opens a Redis key for read and write access.
    pub fn open_key_writable(&self, key: &str) -> RedisKeyWritable {
        RedisKeyWritable::open(self.ctx, key)
    }

    /// Tells Redis that we're about to reply with an (Redis) array.
    ///
    /// Used by invoking once with the expected length and then calling any
    /// combination of the other reply_* methods exactly that number of times.
    pub fn reply_array(&self, len: i64) -> Result<(), CellError> {
        handle_status(
            raw::reply_with_array(self.ctx, len as c_long),
            "Could not reply with long",
        )
    }

    pub fn reply_integer(&self, integer: i64) -> Result<(), CellError> {
        handle_status(
            raw::reply_with_long_long(self.ctx, integer as c_longlong),
            "Could not reply with longlong",
        )
    }

    pub fn reply_string(&self, message: &str) -> Result<(), CellError> {
        let redis_str = self.create_string(message);
        handle_status(
            raw::reply_with_string(self.ctx, redis_str.str_inner),
            "Could not reply with string",
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyMode {
    Read,
    ReadWrite,
}

/// `RedisKey` is an abstraction over a Redis key that allows readonly
/// operations.
///
/// Its primary function is to ensure the proper deallocation of resources when
/// it goes out of scope. Redis normally requires that keys be managed manually
/// by explicitly freeing them when you're done. This can be a risky prospect,
/// especially with mechanics like Rust's `?` operator, so we ensure fault-free
/// operation through the use of the Drop trait.
#[derive(Debug)]
pub struct RedisKey {
    ctx:       *mut raw::RedisModuleCtx,
    key_inner: *mut raw::RedisModuleKey,
    key_str:   RedisString,
}

impl RedisKey {
    fn open(ctx: *mut raw::RedisModuleCtx, key: &str) -> RedisKey {
        let key_str = RedisString::create(ctx, key);
        let key_inner = raw::open_key(ctx, key_str.str_inner, to_raw_mode(KeyMode::Read));
        RedisKey {
            ctx,
            key_inner,
            key_str,
        }
    }

    /// Detects whether the key pointer given to us by Redis is null.
    pub fn is_null(&self) -> bool {
        let null_key: *mut raw::RedisModuleKey = ptr::null_mut();
        self.key_inner == null_key
    }

    pub fn read(&self) -> Result<Option<String>, CellError> {
        let val = if self.is_null() {
            None
        } else {
            Some(read_key(self.key_inner)?)
        };
        Ok(val)
    }
}

impl Drop for RedisKey {
    // Frees resources appropriately as a RedisKey goes out of scope.
    fn drop(&mut self) {
        raw::close_key(self.key_inner);
    }
}

/// `RedisKeyWritable` is an abstraction over a Redis key that allows read and
/// write operations.
pub struct RedisKeyWritable {
    ctx:       *mut raw::RedisModuleCtx,
    key_inner: *mut raw::RedisModuleKey,

    // The Redis string
    //
    // This field is needed on the struct so that its Drop implementation gets
    // called when it goes out of scope.
    #[allow(dead_code)]
    key_str: RedisString,
}

impl RedisKeyWritable {
    fn open(ctx: *mut raw::RedisModuleCtx, key: &str) -> RedisKeyWritable {
        let key_str = RedisString::create(ctx, key);
        let key_inner =
            raw::open_key(ctx, key_str.str_inner, to_raw_mode(KeyMode::ReadWrite));
        RedisKeyWritable {
            ctx,
            key_inner,
            key_str,
        }
    }

    /// Detects whether the value stored in a Redis key is empty.
    ///
    /// Note that an empty key can be reliably detected by looking for a null
    /// as you open the key in read mode, but when asking for write Redis
    /// returns a non-null pointer to allow us to write to even an empty key,
    /// so we have to check the key's value instead.
    pub fn is_empty(&self) -> Result<bool, CellError> {
        match self.read()? {
            Some(s) => match s.as_str() {
                "" => Ok(true),
                _ => Ok(false),
            },
            _ => Ok(false),
        }
    }

    pub fn read(&self) -> Result<Option<String>, CellError> {
        Ok(Some(read_key(self.key_inner)?))
    }

    pub fn set_expire(&self, expire: time::Duration) -> Result<(), CellError> {
        match raw::set_expire(self.key_inner, expire.num_milliseconds()) {
            raw::Status::Ok => Ok(()),

            // Error may occur if the key wasn't open for writing or is an
            // empty key.
            raw::Status::Err => Err(error!("Error while setting key expire")),
        }
    }

    pub fn write(&self, val: &str) -> Result<(), CellError> {
        let val_str = RedisString::create(self.ctx, val);
        match raw::string_set(self.key_inner, val_str.str_inner) {
            raw::Status::Ok => Ok(()),
            raw::Status::Err => Err(error!("Error while setting key")),
        }
    }
}

impl Drop for RedisKeyWritable {
    // Frees resources appropriately as a RedisKey goes out of scope.
    fn drop(&mut self) {
        raw::close_key(self.key_inner);
    }
}

/// `RedisString` is an abstraction over a Redis string.
///
/// Its primary function is to ensure the proper deallocation of resources when
/// it goes out of scope. Redis normally requires that strings be managed
/// manually by explicitly freeing them when you're done. This can be a risky
/// prospect, especially with mechanics like Rust's `?` operator, so we ensure
/// fault-free operation through the use of the Drop trait.
#[derive(Debug)]
pub struct RedisString {
    ctx:       *mut raw::RedisModuleCtx,
    str_inner: *mut raw::RedisModuleString,
}

impl RedisString {
    fn create(ctx: *mut raw::RedisModuleCtx, s: &str) -> RedisString {
        let str_inner = raw::create_string(ctx, format!("{}\0", s).as_ptr(), s.len());
        RedisString { ctx, str_inner }
    }
}

impl Drop for RedisString {
    // Frees resources appropriately as a RedisString goes out of scope.
    fn drop(&mut self) {
        raw::free_string(self.ctx, self.str_inner);
    }
}

fn handle_status(status: raw::Status, message: &str) -> Result<(), CellError> {
    match status {
        raw::Status::Ok => Ok(()),
        raw::Status::Err => Err(error!(message)),
    }
}

fn manifest_redis_reply(
    reply: *mut raw::RedisModuleCallReply,
) -> Result<Reply, CellError> {
    match raw::call_reply_type(reply) {
        raw::ReplyType::Integer => Ok(Reply::Integer(raw::call_reply_integer(reply))),
        raw::ReplyType::Nil => Ok(Reply::Nil),
        raw::ReplyType::String => {
            let mut length: size_t = 0;
            let bytes = raw::call_reply_string_ptr(reply, &mut length);
            from_byte_string(bytes, length)
                .map(Reply::String)
                .map_err(CellError::from)
        }
        raw::ReplyType::Unknown => Ok(Reply::Unknown),

        // TODO: I need to actually extract the error from Redis here. Also, it
        // should probably be its own non-generic variety of CellError.
        raw::ReplyType::Error => Err(error!("Redis replied with an error.")),

        other => Err(error!("Don't yet handle Redis type: {:?}", other)),
    }
}

fn manifest_redis_string(
    redis_str: *mut raw::RedisModuleString,
) -> Result<String, string::FromUtf8Error> {
    let mut length: size_t = 0;
    let bytes = raw::string_ptr_len(redis_str, &mut length);
    from_byte_string(bytes, length)
}

fn parse_args(
    argv: *mut *mut raw::RedisModuleString,
    argc: c_int,
) -> Result<Vec<String>, string::FromUtf8Error> {
    let mut args: Vec<String> = Vec::with_capacity(argc as usize);
    for i in 0..argc {
        let redis_str = unsafe { *argv.offset(i as isize) };
        args.push(manifest_redis_string(redis_str)?);
    }
    Ok(args)
}

fn from_byte_string(
    byte_str: *const u8,
    length: size_t,
) -> Result<String, string::FromUtf8Error> {
    let mut vec_str: Vec<u8> = Vec::with_capacity(length as usize);
    for j in 0..length {
        let byte: u8 = unsafe { *byte_str.offset(j as isize) };
        vec_str.insert(j, byte);
    }

    String::from_utf8(vec_str)
}

fn read_key(key: *mut raw::RedisModuleKey) -> Result<String, string::FromUtf8Error> {
    let mut length: size_t = 0;
    from_byte_string(
        raw::string_dma(key, &mut length, raw::KeyMode::READ),
        length,
    )
}

fn to_raw_mode(mode: KeyMode) -> raw::KeyMode {
    match mode {
        KeyMode::Read => raw::KeyMode::READ,
        KeyMode::ReadWrite => raw::KeyMode::READ | raw::KeyMode::WRITE,
    }
}
