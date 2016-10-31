#![allow(dead_code)]

extern crate libc;

use libc::{c_int, c_longlong, size_t};

// Rust can't link against C macros (#define) so we just redefine them here.
// There's a ~0 chance that any of these will ever change so it's pretty safe.
pub const REDISMODULE_APIVER_1: c_int = 1;

#[derive(PartialEq)]
pub enum Status {
    Ok = 0,
    Err = 1,
}

pub const REDISMODULE_READ: c_int = (1 << 0);
pub const REDISMODULE_WRITE: c_int = (1 << 1);

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RedisModuleCallReply;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RedisModuleCtx;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RedisModuleKey;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RedisModuleString;

pub type RedisModuleCmdFunc = extern "C" fn(ctx: *mut RedisModuleCtx,
                                            argv: *mut RedisModuleString,
                                            argc: c_int)
                                            -> Status;

// Redis doesn't make this easy for us by exporting a library, so instead what
// we do is bake redismodule.h's symbols into a library of our construction
// during build and link against that. See build.rs for details.
#[allow(improper_ctypes)]
#[link(name = "redismodule")]
extern "C" {
    pub fn Export_RedisModule_Init(ctx: *mut RedisModuleCtx,
                                   modulename: *const u8,
                                   module_version: c_int,
                                   api_version: c_int)
                                   -> Status;

    pub static RedisModule_Call: extern "C" fn(ctx: *mut RedisModuleCtx,
                                               cmdname: *const u8,
                                               fmts: &[*const u8])
                                               -> *mut RedisModuleCallReply;

    pub static RedisModule_FreeCallReply: extern "C" fn(reply: *mut RedisModuleCallReply);

    pub static RedisModule_CallReplyInteger: extern "C" fn(reply: *mut RedisModuleCallReply)
                                                           -> c_longlong;

    pub static RedisModule_CloseKey: extern "C" fn(kp: *mut RedisModuleKey);

    pub static RedisModule_CreateCommand: extern "C" fn(ctx: *mut RedisModuleCtx,
                                                        name: *const u8,
                                                        cmdfunc: Option<RedisModuleCmdFunc>,
                                                        strflags: *const u8,
                                                        firstkey: c_int,
                                                        lastkey: c_int,
                                                        keystep: c_int)
                                                        -> Status;

    pub static RedisModule_CreateString: extern "C" fn(ctx: *mut RedisModuleCtx,
                                                       ptr: *const u8,
                                                       len: size_t)
                                                       -> *mut RedisModuleString;

    pub static RedisModule_GetSelectedDb: extern "C" fn(ctx: *mut RedisModuleCtx) -> c_int;

    pub static RedisModule_OpenKey: extern "C" fn(ctx: *mut RedisModuleCtx,
                                                  keyname: *mut RedisModuleString,
                                                  mode: c_int)
                                                  -> *mut RedisModuleKey;

    pub static RedisModule_ReplyWithLongLong: extern "C" fn(ctx: *mut RedisModuleCtx,
                                                            ll: c_longlong)
                                                            -> Status;

    pub static RedisModule_ReplyWithString: extern "C" fn(ctx: *mut RedisModuleCtx,
                                                          str: *mut RedisModuleString)
                                                          -> Status;

    pub static RedisModule_StringSet: extern "C" fn(key: *mut RedisModuleKey,
                                                    str: *mut RedisModuleString)
                                                    -> Status;
}
