extern crate libc;
extern crate time;

#[macro_use]
mod macros;

pub mod error;
mod redis;
pub mod throttle;

use error::ThrottleError;
use libc::c_int;
use redis::Command;
use redis::raw;
use throttle::store;

const MODULE_NAME: &'static str = "redis-throttle";
const MODULE_VERSION: c_int = 1;

// ThrottleCommand provides GCRA rate limiting as a command in Redis.
struct ThrottleCommand {
}

impl Command for ThrottleCommand {
    // Should return the name of the command to be registered.
    fn name(&self) -> &'static str {
        "throttle"
    }

    // Run the command.
    fn run(&self, r: redis::Redis, args: &[&str]) -> Result<(), ThrottleError> {
        if args.len() != 5 && args.len() != 6 {
            return Err(error!("Usage: throttle <key> <max_burst> <count> <period> \
                               [<quantity>]"));
        }

        // the first argument is command name "throttle" (ignore it)
        let key = args[1];
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
        let mut store = store::InternalRedisStore::new(&r);
        let rate = throttle::Rate::per_period(count, time::Duration::seconds(period));
        let mut limiter = throttle::RateLimiter::new(&mut store,
                                                     throttle::RateQuota {
                                                         max_burst: max_burst,
                                                         max_rate: rate,
                                                     });

        let (throttled, rate_limit_result) = try!(limiter.rate_limit(key, quantity));

        // Reply with an array containing rate limiting results. Note that
        // Redis' support for interesting data types is quite weak, so we have
        // to jam a few square pegs into round holes. It's a little messy, but
        // the interface comes out as pretty workable.
        try!(r.reply_array(5));
        try!(r.reply_integer(if throttled { 1 } else { 0 }));
        try!(r.reply_integer(rate_limit_result.limit));
        try!(r.reply_integer(rate_limit_result.remaining));
        try!(r.reply_integer(rate_limit_result.retry_after.num_seconds()));
        try!(r.reply_integer(rate_limit_result.reset_after.num_seconds()));

        Ok(())
    }

    // Should return any flags to be registered with the name as a string
    // separated list. See the Redis module API documentation for a complete
    // list of the ones that are available.
    fn str_flags(&self) -> &'static str {
        "write"
    }
}

#[allow(non_snake_case)]
#[allow(unused_variables)]
#[no_mangle]
pub extern "C" fn Throttle_RedisCommand(ctx: *mut raw::RedisModuleCtx,
                                        argv: *mut *mut raw::RedisModuleString,
                                        argc: c_int)
                                        -> raw::Status {
    Command::harness(&ThrottleCommand {}, ctx, argv, argc)
}

#[allow(non_snake_case)]
#[allow(unused_variables)]
#[no_mangle]
pub extern "C" fn RedisModule_OnLoad(ctx: *mut raw::RedisModuleCtx,
                                     argv: *mut *mut raw::RedisModuleString,
                                     argc: c_int)
                                     -> raw::Status {
    if raw::init(ctx,
                 format!("{}\0", MODULE_NAME).as_ptr(),
                 MODULE_VERSION,
                 raw::REDISMODULE_APIVER_1) == raw::Status::Err {
        return raw::Status::Err;
    }

    let command = ThrottleCommand {};
    if raw::create_command(ctx,
                           format!("{}\0", command.name()).as_ptr(),
                           Some(Throttle_RedisCommand),
                           format!("{}\0", command.str_flags()).as_ptr(),
                           0,
                           0,
                           0) == raw::Status::Err {
        return raw::Status::Err;
    }

    return raw::Status::Ok;
}

fn parse_i64(arg: &str) -> Result<i64, ThrottleError> {
    arg.parse::<i64>()
        .map_err(|_| error!("Couldn't parse as integer: {}", arg))
}
