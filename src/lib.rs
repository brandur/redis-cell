#[macro_use]
extern crate bitflags;
extern crate libc;
extern crate time;

#[macro_use]
mod macros;

pub mod cell;
pub mod error;
mod redis;

use cell::store;
use error::CellError;
use libc::c_int;
use redis::raw;
use redis::Command;

const MODULE_NAME: &str = "redis-cell";
const MODULE_VERSION: c_int = 1;

// ThrottleCommand provides GCRA rate limiting as a command in Redis.
struct ThrottleCommand {}

impl Command for ThrottleCommand {
    // Should return the name of the command to be registered.
    fn name(&self) -> &'static str {
        "cl.throttle"
    }

    // Run the command.
    fn run(&self, r: redis::Redis, args: &[&str]) -> Result<(), CellError> {
        if args.len() != 5 && args.len() != 6 {
            return Err(error!(
                "Usage: {} <key> <max_burst> <count per period> \
                 <period> [<quantity>]",
                self.name()
            ));
        }

        // the first argument is command name "cl.throttle" (ignore it)
        let key = args[1];
        let max_burst = parse_i64(args[2])?;
        let count = parse_i64(args[3])?;
        let period = parse_i64(args[4])?;
        let quantity = match args.get(5) {
            Some(n) => parse_i64(n)?,
            None => 1,
        };

        // We reinitialize a new store and rate limiter every time this command
        // is run, but these structures don't have a huge overhead to them so
        // it's not that big of a problem.
        let mut store = store::InternalRedisStore::new(&r);
        let rate = cell::Rate::per_period(count, time::Duration::seconds(period));
        let mut limiter = cell::RateLimiter::new(
            &mut store,
            &cell::RateQuota {
                max_burst,
                max_rate: rate,
            },
        );

        let (throttled, rate_limit_result) = limiter.rate_limit(key, quantity)?;

        // If either time had a partial component, but it up to the next full
        // second because otherwise a fast-paced caller could try again too
        // early.
        let mut retry_after = rate_limit_result.retry_after.as_seconds_f64() as i64;
        if rate_limit_result.retry_after.subsec_milliseconds() > 0 {
            retry_after += 1
        }
        let mut reset_after = rate_limit_result.reset_after.as_seconds_f64() as i64;
        if rate_limit_result.reset_after.subsec_milliseconds() > 0 {
            reset_after += 1
        }

        // Reply with an array containing rate limiting results. Note that
        // Redis' support for interesting data types is quite weak, so we have
        // to jam a few square pegs into round holes. It's a little messy, but
        // the interface comes out as pretty workable.
        r.reply_array(5)?;
        r.reply_integer(if throttled { 1 } else { 0 })?;
        r.reply_integer(rate_limit_result.limit)?;
        r.reply_integer(rate_limit_result.remaining)?;
        r.reply_integer(retry_after)?;
        r.reply_integer(reset_after)?;

        // Tell Redis that it's okay to replicate the command with the same
        // parameters out to replicas.
        r.replicate_verbatim()?;

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
pub extern "C" fn Throttle_RedisCommand(
    ctx: *mut raw::RedisModuleCtx,
    argv: *mut *mut raw::RedisModuleString,
    argc: c_int,
) -> raw::Status {
    <dyn Command>::harness(&ThrottleCommand {}, ctx, argv, argc)
}

#[allow(non_snake_case)]
#[allow(unused_variables)]
#[no_mangle]
pub extern "C" fn RedisModule_OnLoad(
    ctx: *mut raw::RedisModuleCtx,
    argv: *mut *mut raw::RedisModuleString,
    argc: c_int,
) -> raw::Status {
    if raw::init(
        ctx,
        format!("{}\0", MODULE_NAME).as_ptr(),
        MODULE_VERSION,
        raw::REDISMODULE_APIVER_1,
    ) == raw::Status::Err
    {
        return raw::Status::Err;
    }

    let command = ThrottleCommand {};
    if raw::create_command(
        ctx,
        format!("{}\0", command.name()).as_ptr(),
        Some(Throttle_RedisCommand),
        format!("{}\0", command.str_flags()).as_ptr(),
        1, // firstkey: first argument that's a key
        1, // lastkey: last argument that's a key
        1, // keystep: the step between first and last key
    ) == raw::Status::Err
    {
        return raw::Status::Err;
    }

    raw::Status::Ok
}

fn parse_i64(arg: &str) -> Result<i64, CellError> {
    arg.parse::<i64>()
        .map_err(|_| error!("Couldn't parse as integer: {}", arg))
}
