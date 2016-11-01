extern crate libc;

mod redis;
pub mod throttle;

use libc::c_int;
use redis::Command;
use redis::ffi::*;

const MODULE_NAME: &'static str = "redis-throttle";
const MODULE_VERSION: c_int = 1;

struct ThrottleCommand {
}

impl ThrottleCommand {
    fn name() -> &'static str {
        "throttle"
    }

    fn str_flags() -> &'static str {
        "readonly"
    }
}

impl redis::Command for ThrottleCommand {
    fn run(&self, r: redis::Redis, args: Vec<&str>) {}
}

#[allow(non_snake_case)]
#[allow(unused_variables)]
#[no_mangle]
pub extern "C" fn Throttle_RedisCommand(ctx: *mut RedisModuleCtx,
                                        argv: *mut RedisModuleString,
                                        argc: c_int)
                                        -> Status {
    let key = "throttle";
    let keyStr = RedisModule_CreateString(ctx, format!("{}\0", key).as_ptr(), key.len());
    let keyPtr = RedisModule_OpenKey(ctx, keyStr, KeyMode::Write);

    let val = "val";
    let valStr = RedisModule_CreateString(ctx, format!("{}\0", val).as_ptr(), val.len());

    RedisModule_StringSet(keyPtr, valStr);
    RedisModule_ReplyWithString(ctx, valStr);
    RedisModule_CloseKey(keyPtr);

    return Status::Ok;
}

#[allow(non_snake_case)]
#[allow(unused_variables)]
#[no_mangle]
pub extern "C" fn RedisModule_OnLoad(ctx: *mut RedisModuleCtx,
                                     argv: *mut *mut RedisModuleString,
                                     argc: c_int)
                                     -> Status {
    unsafe {
        if Export_RedisModule_Init(ctx,
                                   format!("{}\0", MODULE_NAME).as_ptr(),
                                   MODULE_VERSION,
                                   REDISMODULE_APIVER_1) == Status::Err {
            return Status::Err;
        }

        if RedisModule_CreateCommand(ctx,
                                     c_str_pointer(ThrottleCommand::name()),
                                     Some(Throttle_RedisCommand),
                                     c_str_pointer(ThrottleCommand::str_flags()),
                                     0,
                                     0,
                                     0) == Status::Err {
            return Status::Err;
        }

    }

    return Status::Ok;
}

fn c_str_pointer(s: &str) -> *const u8 {
    format!("{}\0", s).as_ptr()
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
