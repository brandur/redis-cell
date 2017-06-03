#[macro_export]
macro_rules! redis_error {
    ($message:expr) => {
        CellError::generic($message)
    };
    ($message:expr, $($arg:tt)*) => {
        CellError::generic(format!($message, $($arg)+).as_str())
    }
}

#[macro_export]
macro_rules! redis_log_debug {
    ($logger:expr, $target:expr) => {
        if cfg!(debug_assertions) {
            $logger.log_debug($target)
        }
    };
    ($logger:expr, $target:expr, $($arg:tt)*) => {
        if cfg!(debug_assertions) {
            $logger.log_debug(format!($target, $($arg)+).as_str())
        }
    }
}

#[macro_export]
macro_rules! redis_module {
	( $name: expr, $ver:expr, $( $command_handle:ident ),+ ) => (
	    #[allow(non_snake_case)]
        #[allow(unused_variables)]
        #[no_mangle]
        pub extern "C" fn RedisModule_OnLoad(ctx: *mut ::redis_module_sys::redis::raw::RedisModuleCtx,
                                             argv: *mut *mut ::redis_module_sys::redis::raw::RedisModuleString,
                                             argc: libc::c_int)
                                             -> ::redis_module_sys::redis::raw::Status {
            if ::redis_module_sys::redis::raw::init(ctx,
                         format!("{}\0", stringify!($name)).as_ptr(),
                         $ver,
                         ::redis_module_sys::redis::raw::REDISMODULE_APIVER_1) == ::redis_module_sys::redis::raw::Status::Err {
                return ::redis_module_sys::redis::raw::Status::Err;
            }

            $(
                if $command_handle.register(ctx) ==
                    ::redis_module_sys::redis::raw::Status::Err {
                    return ::redis_module_sys::redis::raw::Status::Err;
                }
            )*

            return ::redis_module_sys::redis::raw::Status::Ok;
        }
	)
}