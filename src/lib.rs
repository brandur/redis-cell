extern crate libc;
use libc::{c_int};

const MODULE_NAME: &'static str = "redis-throttle";
const MODULE_VERSION: c_int = 1;

// Rust can't link against C macros (#define) so we just redefine them here.
// There's a ~0 chance that any of these will ever change so it's pretty safe.
const REDISMODULE_APIVER_1: c_int = 1;
const REDISMODULE_OK: c_int = 0;
const REDISMODULE_ERR: c_int = 1;

#[allow(improper_ctypes)]
#[derive(Clone, Copy)]
#[repr(C)]
pub struct RedisModuleCtx;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RedisModuleString;

type RedisModuleCmdFunc = extern "C" fn(ctx: *mut RedisModuleCtx, argv: *mut RedisModuleString, argc: c_int) -> c_int;

// Redis doesn't make this easy for us by exporting a library, so instead what
// we do is bake redismodule.h's symbols into a library of our construction
// during build and link against that. See build.rs for details.
#[link(name = "redismodule")]
extern {
    fn Export_RedisModule_Init(ctx: *mut RedisModuleCtx,
                               modulename: *const u8, module_version: c_int,
                               api_version: c_int) -> c_int;

    fn RedisModule_CreateCommand(ctx: *mut RedisModuleCtx, name: *const u8,
                                 cmdfunc: RedisModuleCmdFunc,
                                 strflags: *const u8, firstkey: c_int,
                                 lastkey: c_int, keystep: c_int) -> c_int;
}

#[allow(non_snake_case)]
#[allow(unused_variables)]
#[no_mangle]
pub extern "C" fn Throttle_RedisCommand(ctx: *mut RedisModuleCtx,
                                        argv: *mut RedisModuleString,
                                        argc: c_int) -> c_int {
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
                                     Throttle_RedisCommand,
                                     ("readonly\0").as_ptr(), 0, 0, 0)
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
