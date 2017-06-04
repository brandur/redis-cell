extern crate bitflags;

#[macro_use]
extern crate redis_module_sys;

#[macro_use]
extern crate redis_command_gen;

extern crate libc;
extern crate time;

pub mod cell;

use redis_module_sys::redis;
use redis_module_sys::redis::{
    RedisCommand,
    RedisCommandAttrs,
};

use cell::store::InternalRedisStore;
use redis_module_sys::error::CellError;

// ThrottleCommand provides GCRA rate limiting as a command in Redis.
#[derive(RedisCommandAttrs)]
#[command(name = "cl.throttle", flags = "write", static_handle="CL_THROTTLE_COMMAND")]
struct ThrottleCommand;
impl RedisCommand for ThrottleCommand {
    // Run the command.
    fn run(&self, r: redis::Redis, args: &[&str]) -> Result<(), CellError> {
        if args.len() != 5 && args.len() != 6 {
            return Err(redis_error!("Usage: {} <key> <max_burst> <count per period> \
                               <period> [<quantity>]",
                              self.name()));
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
        let mut store = InternalRedisStore::new(&r);
        let rate = cell::Rate::per_period(count, time::Duration::seconds(period));
        let mut limiter = cell::RateLimiter::new(&mut store,
                                                 cell::RateQuota {
                                                     max_burst: max_burst,
                                                     max_rate: rate,
                                                 });

        let (throttled, rate_limit_result) = limiter.rate_limit(key, quantity)?;

        // Reply with an array containing rate limiting results. Note that
        // Redis' support for interesting data types is quite weak, so we have
        // to jam a few square pegs into round holes. It's a little messy, but
        // the interface comes out as pretty workable.
        r.reply_array(5)?;
        r.reply_integer(if throttled { 1 } else { 0 })?;
        r.reply_integer(rate_limit_result.limit)?;
        r.reply_integer(rate_limit_result.remaining)?;
        r.reply_integer(rate_limit_result.retry_after.num_seconds())?;
        r.reply_integer(rate_limit_result.reset_after.num_seconds())?;

        Ok(())
    }
}

redis_module!("redis-cell", 1, CL_THROTTLE_COMMAND);

fn parse_i64(arg: &str) -> Result<i64, CellError> {
    arg.parse::<i64>()
        .map_err(|_| redis_error!("Couldn't parse as integer: {}", arg))
}
