// Allow dead code in here in case I want to publish it as a crate at some
// point.
#![allow(dead_code)]

extern crate libc;

use libc::{c_int, c_long, c_longlong, size_t};

// Rust can't link against C macros (#define) so we just redefine them here.
// There's a ~0 chance that any of these will ever change so it's pretty safe.
pub const REDISMODULE_APIVER_1: c_int = 1;

bitflags! {
    pub struct KeyMode: c_int {
        const READ = 1;
        const WRITE = (1 << 1);
    }
}

#[derive(Debug, PartialEq)]
pub enum ReplyType {
    Unknown = -1,
    String = 0,
    Error = 1,
    Integer = 2,
    Array = 3,
    Nil = 4,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    Ok = 0,
    Err = 1,
}

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

pub type RedisModuleCmdFunc = extern "C" fn(
    ctx: *mut RedisModuleCtx,
    argv: *mut *mut RedisModuleString,
    argc: c_int,
) -> Status;

pub fn init(
    ctx: *mut RedisModuleCtx,
    modulename: *const u8,
    module_version: c_int,
    api_version: c_int,
) -> Status {
    unsafe { Export_RedisModule_Init(ctx, modulename, module_version, api_version) }
}

pub fn call_reply_type(reply: *mut RedisModuleCallReply) -> ReplyType {
    unsafe { RedisModule_CallReplyType(reply) }
}

pub fn free_call_reply(reply: *mut RedisModuleCallReply) {
    unsafe {
        RedisModule_FreeCallReply(reply);
    }
}

pub fn call_reply_integer(reply: *mut RedisModuleCallReply) -> c_longlong {
    unsafe { RedisModule_CallReplyInteger(reply) }
}

pub fn call_reply_string_ptr(
    str: *mut RedisModuleCallReply,
    len: *mut size_t,
) -> *const u8 {
    unsafe { RedisModule_CallReplyStringPtr(str, len) }
}

pub fn close_key(kp: *mut RedisModuleKey) {
    unsafe { RedisModule_CloseKey(kp) }
}

pub fn create_command(
    ctx: *mut RedisModuleCtx,
    name: *const u8,
    cmdfunc: Option<RedisModuleCmdFunc>,
    strflags: *const u8,
    firstkey: c_int,
    lastkey: c_int,
    keystep: c_int,
) -> Status {
    unsafe {
        RedisModule_CreateCommand(
            ctx,
            name,
            cmdfunc,
            strflags,
            firstkey,
            lastkey,
            keystep,
        )
    }
}

pub fn create_string(
    ctx: *mut RedisModuleCtx,
    ptr: *const u8,
    len: size_t,
) -> *mut RedisModuleString {
    unsafe { RedisModule_CreateString(ctx, ptr, len) }
}

pub fn free_string(ctx: *mut RedisModuleCtx, str: *mut RedisModuleString) {
    unsafe { RedisModule_FreeString(ctx, str) }
}

pub fn get_selected_db(ctx: *mut RedisModuleCtx) -> c_int {
    unsafe { RedisModule_GetSelectedDb(ctx) }
}

pub fn log(ctx: *mut RedisModuleCtx, level: *const u8, fmt: *const u8) {
    unsafe { RedisModule_Log(ctx, level, fmt) }
}

pub fn open_key(
    ctx: *mut RedisModuleCtx,
    keyname: *mut RedisModuleString,
    mode: KeyMode,
) -> *mut RedisModuleKey {
    unsafe { RedisModule_OpenKey(ctx, keyname, mode) }
}

pub fn reply_with_array(ctx: *mut RedisModuleCtx, len: c_long) -> Status {
    unsafe { RedisModule_ReplyWithArray(ctx, len) }
}

pub fn reply_with_error(ctx: *mut RedisModuleCtx, err: *const u8) {
    unsafe { RedisModule_ReplyWithError(ctx, err) }
}

pub fn reply_with_long_long(ctx: *mut RedisModuleCtx, ll: c_longlong) -> Status {
    unsafe { RedisModule_ReplyWithLongLong(ctx, ll) }
}

pub fn reply_with_string(
    ctx: *mut RedisModuleCtx,
    str: *mut RedisModuleString,
) -> Status {
    unsafe { RedisModule_ReplyWithString(ctx, str) }
}

// Sets the expiry on a key.
//
// Expire is in milliseconds.
pub fn set_expire(key: *mut RedisModuleKey, expire: c_longlong) -> Status {
    unsafe { RedisModule_SetExpire(key, expire) }
}

pub fn string_dma(
    key: *mut RedisModuleKey,
    len: *mut size_t,
    mode: KeyMode,
) -> *const u8 {
    unsafe { RedisModule_StringDMA(key, len, mode) }
}

pub fn string_ptr_len(str: *mut RedisModuleString, len: *mut size_t) -> *const u8 {
    unsafe { RedisModule_StringPtrLen(str, len) }
}

pub fn string_set(key: *mut RedisModuleKey, str: *mut RedisModuleString) -> Status {
    unsafe { RedisModule_StringSet(key, str) }
}

// Redis doesn't make this easy for us by exporting a library, so instead what
// we do is bake redismodule.h's symbols into a library of our construction
// during build and link against that. See build.rs for details.
#[allow(improper_ctypes)]
#[link(name = "redismodule", kind = "static")]
extern "C" {
    pub fn Export_RedisModule_Init(
        ctx: *mut RedisModuleCtx,
        modulename: *const u8,
        module_version: c_int,
        api_version: c_int,
    ) -> Status;

    static RedisModule_CallReplyType:
        extern "C" fn(reply: *mut RedisModuleCallReply) -> ReplyType;

    static RedisModule_FreeCallReply: extern "C" fn(reply: *mut RedisModuleCallReply);

    static RedisModule_CallReplyInteger:
        extern "C" fn(reply: *mut RedisModuleCallReply) -> c_longlong;

    static RedisModule_CallReplyStringPtr:
        extern "C" fn(str: *mut RedisModuleCallReply, len: *mut size_t) -> *const u8;

    static RedisModule_CloseKey: extern "C" fn(kp: *mut RedisModuleKey);

    static RedisModule_CreateCommand:
        extern "C" fn(
        ctx: *mut RedisModuleCtx,
        name: *const u8,
        cmdfunc: Option<RedisModuleCmdFunc>,
        strflags: *const u8,
        firstkey: c_int,
        lastkey: c_int,
        keystep: c_int,
    ) -> Status;

    static RedisModule_CreateString:
        extern "C" fn(ctx: *mut RedisModuleCtx, ptr: *const u8, len: size_t)
        -> *mut RedisModuleString;

    static RedisModule_FreeString:
        extern "C" fn(ctx: *mut RedisModuleCtx, str: *mut RedisModuleString);

    static RedisModule_GetSelectedDb: extern "C" fn(ctx: *mut RedisModuleCtx) -> c_int;

    static RedisModule_Log:
        extern "C" fn(ctx: *mut RedisModuleCtx, level: *const u8, fmt: *const u8);

    static RedisModule_OpenKey:
        extern "C" fn(
        ctx: *mut RedisModuleCtx,
        keyname: *mut RedisModuleString,
        mode: KeyMode,
    ) -> *mut RedisModuleKey;

    static RedisModule_ReplyWithArray:
        extern "C" fn(ctx: *mut RedisModuleCtx, len: c_long) -> Status;

    static RedisModule_ReplyWithError:
        extern "C" fn(ctx: *mut RedisModuleCtx, err: *const u8);

    static RedisModule_ReplyWithLongLong:
        extern "C" fn(ctx: *mut RedisModuleCtx, ll: c_longlong) -> Status;

    static RedisModule_ReplyWithString:
        extern "C" fn(ctx: *mut RedisModuleCtx, str: *mut RedisModuleString) -> Status;

    static RedisModule_SetExpire:
        extern "C" fn(key: *mut RedisModuleKey, expire: c_longlong) -> Status;

    static RedisModule_StringDMA:
        extern "C" fn(key: *mut RedisModuleKey, len: *mut size_t, mode: KeyMode) -> *const u8;

    static RedisModule_StringPtrLen:
        extern "C" fn(str: *mut RedisModuleString, len: *mut size_t) -> *const u8;

    static RedisModule_StringSet:
        extern "C" fn(key: *mut RedisModuleKey, str: *mut RedisModuleString) -> Status;

    static RedisModule_Call:
        extern "C" fn(
        ctx: *mut RedisModuleCtx,
        cmdname: *const u8,
        fmt: *const u8,
        args: *const *mut RedisModuleString,
    ) -> *mut RedisModuleCallReply;
}

pub mod call1 {
    use redis::raw;

    pub fn call(
        ctx: *mut raw::RedisModuleCtx,
        cmdname: *const u8,
        fmt: *const u8,
        arg0: *mut raw::RedisModuleString,
    ) -> *mut raw::RedisModuleCallReply {
        unsafe { RedisModule_Call(ctx, cmdname, fmt, arg0) }
    }

    #[allow(improper_ctypes)]
    extern "C" {
        pub static RedisModule_Call:
            extern "C" fn(
            ctx: *mut raw::RedisModuleCtx,
            cmdname: *const u8,
            fmt: *const u8,
            arg0: *mut raw::RedisModuleString,
        ) -> *mut raw::RedisModuleCallReply;
    }
}

pub mod call2 {
    use redis::raw;

    pub fn call(
        ctx: *mut raw::RedisModuleCtx,
        cmdname: *const u8,
        fmt: *const u8,
        arg0: *mut raw::RedisModuleString,
        arg1: *mut raw::RedisModuleString,
    ) -> *mut raw::RedisModuleCallReply {
        unsafe { RedisModule_Call(ctx, cmdname, fmt, arg0, arg1) }
    }

    #[allow(improper_ctypes)]
    extern "C" {
        pub static RedisModule_Call:
            extern "C" fn(
            ctx: *mut raw::RedisModuleCtx,
            cmdname: *const u8,
            fmt: *const u8,
            arg0: *mut raw::RedisModuleString,
            arg1: *mut raw::RedisModuleString,
        ) -> *mut raw::RedisModuleCallReply;
    }
}

pub mod call3 {
    use redis::raw;

    pub fn call(
        ctx: *mut raw::RedisModuleCtx,
        cmdname: *const u8,
        fmt: *const u8,
        arg0: *mut raw::RedisModuleString,
        arg1: *mut raw::RedisModuleString,
        arg2: *mut raw::RedisModuleString,
    ) -> *mut raw::RedisModuleCallReply {
        unsafe { RedisModule_Call(ctx, cmdname, fmt, arg0, arg1, arg2) }
    }

    #[allow(improper_ctypes)]
    extern "C" {
        pub static RedisModule_Call:
            extern "C" fn(
            ctx: *mut raw::RedisModuleCtx,
            cmdname: *const u8,
            fmt: *const u8,
            arg0: *mut raw::RedisModuleString,
            arg1: *mut raw::RedisModuleString,
            arg2: *mut raw::RedisModuleString,
        ) -> *mut raw::RedisModuleCallReply;
    }
}
