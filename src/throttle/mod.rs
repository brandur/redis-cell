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
    pub count: i64,
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
        Rate {
            count: n,
            period: div_durations(make_duration(1), make_duration(n)),
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

        let mut i = 0;
        let mut limited = false;
        let mut new_tat: time::Tm;
        let mut ttl: time::Duration;
        let mut rlc = RateLimitResult {
            limit: self.limit,
            remaining: 0,
            reset_after: time::Duration::seconds(-1),
            retry_after: time::Duration::seconds(-1),
        };

        loop {
            // tat refers to the theoretical arrival time that would be expected
            // from equally spaced requests at exactly the rate limit.
            let (tat_val, now) = try!(self.store.get_with_time(key));

            let tat = if tat_val == -1 {
                time::now()
            } else {
                time::at(time::Timespec {
                    sec: tat_val,
                    nsec: 0,
                })
            };

            let increment = time::Duration::nanoseconds(self.emission_interval
                .num_nanoseconds()
                .unwrap() * quantity);

            new_tat = if now > tat {
                now + increment
            } else {
                tat + increment
            };

            // Block the request if the next permitted time is in the future
            let allow_at = new_tat - self.delay_variation_tolerance;
            let diff = now - allow_at;
            if diff.num_seconds() < 0 {
                if increment <= self.delay_variation_tolerance {
                    rlc.retry_after = -diff;
                }
                ttl = tat - now;
                limited = true;
                break;
            }

            ttl = new_tat - now;

            let updated_res = if tat_val == -1 {
                self.store.set_if_not_exists_with_ttl(key, nano_seconds(new_tat), ttl)
            } else {
                self.store.compare_and_swap_with_ttl(key, tat_val, nano_seconds(new_tat), ttl)
            };

            if updated_res.is_ok() {
                return Ok((false, rlc));
            }

            if updated_res.unwrap() {
                break;
            }

            i += 1;
            if i > MAX_CAS_ATTEMPTS {
                return Err(ThrottleError::generic(format!("Failed to store updated rate limit \
                                                           data for key {} after {} attempts.",
                                                          key,
                                                          i)
                    .as_str()));
            }
        }

        let next = self.delay_variation_tolerance - ttl;
        if next > -self.emission_interval {
            // TODO: check that num_seconds is actually what we want here
            rlc.remaining = div_durations(next, self.emission_interval).num_seconds();
        }
        rlc.reset_after = ttl;

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

fn nano_seconds(x: time::Tm) -> i64 {
    let ts = x.to_timespec();
    ts.sec * (10 as i64).pow(9) + (ts.nsec as i64)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
