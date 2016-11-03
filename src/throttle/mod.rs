// Remove as soon as it's not annoying as fuck to have this (i.e. we've
// implemented more of the library).
#![allow(dead_code)]

extern crate time;

pub mod store;

use error::ThrottleError;

// Maximum number of times to retry set_if_not_exists/compare_and_swap
// operations before returning an error.
const MAX_CAS_ATTEMPTS: i64 = 10;

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
        self.store.log_debug("");
        self.store.log_debug("-----");
        self.store.log_debug(format!("bucket = {}", key).as_str());
        self.store.log_debug(format!("quantity = {}", key).as_str());
        self.store.log_debug(format!("delay_variation_tolerance = {}ms",
                                     self.delay_variation_tolerance.num_milliseconds())
            .as_str());
        self.store.log_debug(format!("emission_interval = {}ms",
                                     self.emission_interval.num_milliseconds())
            .as_str());

        let mut rlc = RateLimitResult {
            limit: self.limit,
            remaining: 0,
            retry_after: time::Duration::seconds(-1),
            reset_after: time::Duration::seconds(-1),
        };

        // tat refers to the theoretical arrival time that would be expected
        // from equally spaced requests at exactly the rate limit.
        let (tat_val, now) = try!(self.store.get_with_time(key));

        let tat = if tat_val == -1 {
            let tat = now;
            self.store
                .log_debug(format!("TAT is NOW: {} (-1 from store)", tat.rfc3339()).as_str());
            tat
        } else {
            let ns = (10 as i64).pow(9);
            let tat = time::at(time::Timespec {
                sec: tat_val / ns,
                nsec: (tat_val % ns) as i32,
            });
            self.store.log_debug(format!("TAT is: {} (now is {})", tat.rfc3339(), now.rfc3339())
                .as_str());
            tat
        };

        let increment = time::Duration::nanoseconds(self.emission_interval
            .num_nanoseconds()
            .unwrap() * quantity);
        self.store
            .log_debug(format!("TAT increment is: {}ms", increment.num_milliseconds()).as_str());

        let new_tat = if now > tat {
            now + increment
        } else {
            tat + increment
        };
        self.store.log_debug(format!("New TAT is: {}", new_tat.rfc3339()).as_str());

        // Block the request if the next permitted time is in the future.
        let allow_at = new_tat - self.delay_variation_tolerance;
        let diff = now - allow_at;
        if diff.num_seconds() < 0 {
            self.store.log_debug(format!("Next allowed request is in the future. Diff: {}",
                                         diff.num_milliseconds())
                .as_str());

            if increment <= self.delay_variation_tolerance {
                rlc.retry_after = -diff;
            }

            let ttl = tat - now;
            update_results(self, &mut rlc, ttl);
            return Ok((true, rlc));
        }

        let ttl = new_tat - now;
        self.store.log_debug(format!("Allowed. New TAT: {} New TTL: {}ms",
                                     new_tat.rfc3339(),
                                     diff.num_milliseconds())
            .as_str());
        self.store.log_debug(format!("Setting: {}", nano_seconds(new_tat)).as_str());

        // TODO: what do we do about updated here? Appears to have been used
        // for a loop decision.
        //
        // Later: that's because it's trying to detect whether another process
        // has modified it while the rate limit logic has been running.
        let _ = if tat_val == -1 {
            try!(self.store.set_if_not_exists_with_ttl(key, nano_seconds(new_tat), ttl))
        } else {
            try!(self.store.compare_and_swap_with_ttl(key, tat_val, nano_seconds(new_tat), ttl))
        };

        update_results(self, &mut rlc, ttl);
        Ok((false, rlc))
    }
}

fn update_results<T: store::Store>(limiter: &RateLimiter<T>,
                                   rlc: &mut RateLimitResult,
                                   ttl: time::Duration) {
    let next = limiter.delay_variation_tolerance - ttl;
    if next > -limiter.emission_interval {
        rlc.remaining = (next.num_microseconds().unwrap() as f64 /
                         limiter.emission_interval
            .num_microseconds()
            .unwrap() as f64) as i64;
    }
    rlc.reset_after = ttl;
}

pub struct RateQuota {
    pub max_burst: i64,
    pub max_rate: Rate,
}

fn div_durations(x: time::Duration, y: time::Duration) -> time::Duration {
    time::Duration::nanoseconds(x.num_nanoseconds().unwrap() / y.num_nanoseconds().unwrap())
}

fn nano_seconds(x: time::Tm) -> i64 {
    let ts = x.to_timespec();
    ts.sec * (10 as i64).pow(9) + (ts.nsec as i64)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
