extern crate libc;
extern crate time;

pub mod error;
mod redis;
pub mod throttle;

use error::ThrottleError;
use libc::c_int;
use redis::raw::*;
use redis::store::RedisStore;

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
        if args.len() != 5 && args.len() != 6 {
            return Err(ThrottleError::generic("Usage: throttle <bucket> <max_burst> \
                                               <count> <period> [<quantity>]"));
        }

        let parse_i64 = |arg: &str| -> Result<i64, ThrottleError> {
            arg.parse::<i64>()
                .map_err(|_| {
                    ThrottleError::generic(format!("Couldn't parse as integer: {}", arg).as_str())
                })
        };

        // the first argument is command name "throttle" (ignore it)
        let bucket = args[1];
        let max_burst = try!(parse_i64(args[2]));
        let count = try!(parse_i64(args[3]));
        let period = try!(parse_i64(args[4]));
        let quantity = match args.get(5) {
            Some(n) => try!(parse_i64(n)),
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

        let (throttled, rate_limit_result) = try!(limiter.rate_limit(bucket, quantity));

        // Reply with an array containing rate limiting results. Note that
        // Redis' support for interesting data types is quite weak, so we have
        // to jam a few square pegs into round holes. It's a little messy, but
        // the interface comes out as pretty workable.
        try!(r.reply_array(5));
        try!(r.reply_integer(if !throttled { 1 } else { 0 }));
        try!(r.reply_integer(rate_limit_result.limit));
        try!(r.reply_integer(rate_limit_result.remaining));
        try!(r.reply_integer(rate_limit_result.reset_after.num_seconds()));
        try!(r.reply_integer(rate_limit_result.retry_after.num_seconds()));

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
