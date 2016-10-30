extern crate libc;

mod redis;

use libc::{c_int, c_longlong};
use redis::*;

const MODULE_NAME: &'static str = "redis-throttle";
const MODULE_VERSION: c_int = 1;

#[allow(non_snake_case)]
#[allow(unused_variables)]
#[no_mangle]
pub extern "C" fn Throttle_RedisCommand(ctx: *mut RedisModuleCtx,
                                        argv: *mut RedisModuleString,
                                        argc: c_int) -> c_int {
    println!("hello from throttle");
    RedisModule_ReplyWithLongLong(ctx,
                                  RedisModule_GetSelectedDb(ctx) as c_longlong);
    return REDISMODULE_OK;
}

#[allow(non_snake_case)]
#[allow(unused_variables)]
#[no_mangle]
pub extern "C" fn RedisModule_OnLoad(ctx: *mut RedisModuleCtx,
                                     argv: *mut *mut RedisModuleString,
                                     argc: c_int) -> c_int {
    unsafe {
        if Export_RedisModule_Init(ctx,
                                   format!("{}{}", MODULE_NAME, "\0").as_ptr(),
                                   MODULE_VERSION, REDISMODULE_APIVER_1)
                                   == REDISMODULE_ERR {
            return REDISMODULE_ERR;
        }

        if RedisModule_CreateCommand(ctx, "throttle\0".as_ptr(),
                                     Some(Throttle_RedisCommand),
                                     "readonly\0".as_ptr(), 0, 0, 0)
                                     == REDISMODULE_ERR {
            return REDISMODULE_ERR;
        }
    }

    return REDISMODULE_OK;
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
