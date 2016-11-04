// Remove as soon as it's not annoying as fuck to have this (i.e. we've
// implemented more of the library).
#![allow(dead_code)]

extern crate time;

pub mod store;

use error::ThrottleError;

// Maximum number of times to retry set_if_not_exists/compare_and_swap
// operations before returning an error.
const MAX_CAS_ATTEMPTS: i64 = 5;

pub struct Rate {
    pub period: time::Duration,
}

impl Rate {
    pub fn per_day(n: i64) -> Rate {
        Rate::per_time(n, |n: i64| -> time::Duration { time::Duration::days(n) })
    }

    pub fn per_hour(n: i64) -> Rate {
        Rate::per_time(n, |n: i64| -> time::Duration { time::Duration::hours(n) })
    }

    pub fn per_minute(n: i64) -> Rate {
        Rate::per_time(n, |n: i64| -> time::Duration { time::Duration::minutes(n) })
    }

    pub fn per_second(n: i64) -> Rate {
        Rate::per_time(n, |n: i64| -> time::Duration { time::Duration::seconds(n) })
    }

    pub fn per_time<F>(n: i64, make_duration: F) -> Rate
        where F: Fn(i64) -> time::Duration
    {
        let duration_ns = make_duration(1).num_nanoseconds().unwrap();
        let count_ns = time::Duration::seconds(n).num_nanoseconds().unwrap();
        Rate {
            period:
                time::Duration::milliseconds((((duration_ns as f64) / (count_ns as f64)) as i64) *
                                             1000),
        }
    }
}

pub struct RateLimitResult {
    pub limit: i64,
    pub remaining: i64,
    pub reset_after: time::Duration,
    pub retry_after: time::Duration,
}

pub struct RateLimiter<T: store::Store> {
    pub store: T,

    /// Think of the DVT as our flexibility: how far can you deviate from the
    /// nominal equally spaced schedule? If you like leaky buckets, think about
    /// it as the size of your bucket.
    delay_variation_tolerance: time::Duration,

    /// Think of the emission interval as the time between events in the
    /// nominal equally spaced schedule. If you like leaky buckets, think of it
    /// as how frequently the bucket leaks one unit.
    emission_interval: time::Duration,

    limit: i64,
}

impl<T: store::Store> RateLimiter<T> {
    pub fn new(store: T, quota: RateQuota) -> RateLimiter<T> {
        RateLimiter {
            delay_variation_tolerance: time::Duration::nanoseconds(quota.max_rate
                .period
                .num_nanoseconds()
                .unwrap() *
                                                                   (quota.max_burst + 1)),
            emission_interval: quota.max_rate.period,
            limit: quota.max_burst + 1,
            store: store,
        }
    }

    /// RateLimit checks whether a particular key has exceeded a rate limit. It
    /// also returns a RateLimitResult to provide additional information about
    /// the state of the RateLimiter.
    ///
    /// If the rate limit has not been exceeded, the underlying storage is
    /// updated by the supplied quantity. For example, a quantity of 1 might be
    /// used to rate limit a single request while a greater quantity could rate
    /// limit based on the size of a file upload in megabytes. If quantity is
    /// 0, no update is performed allowing you to "peek" at the state of the
    /// RateLimiter for a given key.
    pub fn rate_limit(&self,
                      key: &str,
                      quantity: i64)
                      -> Result<(bool, RateLimitResult), ThrottleError> {
        let mut rlc = RateLimitResult {
            limit: self.limit,
            remaining: 0,
            retry_after: time::Duration::seconds(-1),
            reset_after: time::Duration::seconds(-1),
        };

        let increment = time::Duration::nanoseconds(self.emission_interval
            .num_nanoseconds()
            .unwrap() * quantity);

        log_debug!(self.store, "");
        log_debug!(self.store, "-----");
        log_debug!(self.store, "bucket = {}", key);
        log_debug!(self.store, "quantity = {}", quantity);
        log_debug!(self.store,
                   "delay_variation_tolerance = {}ms",
                   self.delay_variation_tolerance.num_milliseconds());
        log_debug!(self.store,
                   "emission_interval = {}ms",
                   self.emission_interval.num_milliseconds());
        log_debug!(self.store,
                   "tat increment = {}ms (emission_interval * quantity)",
                   increment.num_milliseconds());

        // Rust actually detects that this variable can only ever be assigned
        // once despite our loops and conditions so it doesn't have to be
        // mutable. Amazing.
        let limited: bool;

        let mut ttl: time::Duration;

        // Looping here is not about retrying communication failures, it's
        // about retrying contention. While we're performing our calculations
        // it's possible for another limiter to be doing its own simultaneously
        // and beat us to the punch. In that case only one limiter should win.
        //
        // Note that when running with our internal Redis store (i.e. the
        // normal case for the redis-throttle project) this is actually *not*
        // true because our entire operation will execute atomically.
        let mut i = 0;
        loop {
            log_debug!(self.store, "iteration = {}", i);

            // tat refers to the theoretical arrival time that would be expected
            // from equally spaced requests at exactly the rate limit.
            let (tat_val, now) = try!(self.store.get_with_time(key));

            let tat = match tat_val {
                -1 => now,
                _ => from_nanoseconds(tat_val),
            };
            log_debug!(self.store,
                       "tat = {} (from store = {})",
                       tat.rfc3339(),
                       tat_val);

            let new_tat = if now > tat {
                now + increment
            } else {
                tat + increment
            };
            log_debug!(self.store, "new_tat = {}", new_tat.rfc3339());

            // Block the request if the next permitted time is in the future.
            let allow_at = new_tat - self.delay_variation_tolerance;
            let diff = now - allow_at;
            log_debug!(self.store,
                       "diff = {}ms (now - allow_at)",
                       diff.num_milliseconds());

            if diff.num_seconds() < 0 {
                log_debug!(self.store,
                           "BLOCKED retry_after = {}",
                           -diff.num_milliseconds());

                if increment <= self.delay_variation_tolerance {
                    rlc.retry_after = -diff;
                }

                limited = true;
                ttl = tat - now;
                break;
            }

            ttl = new_tat - now;
            log_debug!(self.store, "ALLOWED");

            // If the key was originally missing, set it if if doesn't exist.
            // If it was there, try to compare and swap.
            //
            // Both of these cases are designed to work around the fact that
            // another limiter could be running in parallel.
            let updated = if tat_val == -1 {
                try!(self.store.set_if_not_exists_with_ttl(key, nanoseconds(new_tat), ttl))
            } else {
                try!(self.store.compare_and_swap_with_ttl(key, tat_val, nanoseconds(new_tat), ttl))
            };

            if updated {
                limited = false;
                break;
            }

            i += 1;
            if i > MAX_CAS_ATTEMPTS {
                return Err(error!("Failed to update rate limit after \
                                                           {} attempts",
                                  i));
            }
        }

        let next = self.delay_variation_tolerance - ttl;
        if next > -self.emission_interval {
            rlc.remaining = (next.num_microseconds().unwrap() as f64 /
                             self.emission_interval
                .num_microseconds()
                .unwrap() as f64) as i64;
        }
        rlc.reset_after = ttl;

        log_debug!(self.store,
                   "limit = {} remaining = {}",
                   self.limit,
                   rlc.remaining);
        log_debug!(self.store, "reset_after = {}ms", ttl.num_milliseconds());

        Ok((limited, rlc))
    }
}

pub struct RateQuota {
    pub max_burst: i64,
    pub max_rate: Rate,
}

fn div_durations(x: time::Duration, y: time::Duration) -> time::Duration {
    time::Duration::nanoseconds(x.num_nanoseconds().unwrap() / y.num_nanoseconds().unwrap())
}

fn from_nanoseconds(x: i64) -> time::Tm {
    let ns = (10 as i64).pow(9);
    time::at(time::Timespec {
        sec: x / ns,
        nsec: (x % ns) as i32,
    })
}

fn nanoseconds(x: time::Tm) -> i64 {
    let ts = x.to_timespec();
    ts.sec * (10 as i64).pow(9) + (ts.nsec as i64)
}

// fn update_results<T: store::Store>(limiter: &RateLimiter<T>,
// rlc: &mut RateLimitResult,
// ttl: time::Duration) {
// }
//

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
