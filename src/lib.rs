extern crate libc;
extern crate time;

pub mod error;
mod redis;
pub mod throttle;

use error::ThrottleError;
use libc::c_int;
use redis::raw::*;
use redis::store::RedisStore;
use std::error::Error;

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
    fn run(&self, r: redis::Redis, args: &[&str]) -> redis::CommandResult {
        if args.len() != 4 && args.len() != 5 {
            return Err(ThrottleError::generic("throttle command expects either 4 or 5 arguments"));
        }

        let bucket = args[0];
        let max_burst =
            try!(args[1].parse::<i64>().map_err(|e| ThrottleError::generic(e.description())));
        let count =
            try!(args[2].parse::<i64>().map_err(|e| ThrottleError::generic(e.description())));
        let period =
            try!(args[3].parse::<i64>().map_err(|e| ThrottleError::generic(e.description())));
        let quantity = match args.get(4) {
            Some(n) => {
                let quantity = try!(n.parse::<i64>()
                    .map_err(|e| ThrottleError::generic(e.description())));
                quantity
            }
            None => 1,
        };

        // We reinitialize a new store and rate limiter every time this command
        // is run, but these structures don't have a huge overhead to them so
        // it's not that big of a problem.
        let store = RedisStore::new(&r);
        let limiter = throttle::RateLimiter::new(store,
                                                 throttle::RateQuota {
                                                     max_burst: max_burst,
                                                     max_rate: throttle::Rate {
                                                         count: count,
                                                         period: time::Duration::seconds(period),
                                                     },
                                                 });

        let (throttled, _) = try!(limiter.rate_limit(bucket, quantity));

        if throttled {
            try!(r.reply_string("THROTTLED"));
        } else {
            try!(r.reply_string("good"));
        }

        Ok(true)
    }
}

#[allow(non_snake_case)]
#[allow(unused_variables)]
#[no_mangle]
pub extern "C" fn Throttle_RedisCommand(ctx: *mut RedisModuleCtx,
                                        argv: *mut *mut RedisModuleString,
                                        argc: c_int)
                                        -> Status {
    redis::harness_command(&ThrottleCommand {}, ctx, argv, argc)
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
                                     format!("{}\0", ThrottleCommand::name()).as_ptr(),
                                     Some(Throttle_RedisCommand),
                                     format!("{}\0", ThrottleCommand::str_flags()).as_ptr(),
                                     0,
                                     0,
                                     0) == Status::Err {
            return Status::Err;
        }

    }

    return Status::Ok;
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
