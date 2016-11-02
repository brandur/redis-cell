// Remove as soon as it's not annoying as fuck to have this (i.e. we've
// implemented more of the library).
#![allow(dead_code)]

extern crate time;

pub mod store;

use error::ThrottleError;

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
    pub quota: RateQuota,
    pub store: T,
}

impl<T: store::Store> RateLimiter<T> {
    pub fn new(store: T, quota: RateQuota) -> RateLimiter<T> {
        RateLimiter {
            quota: quota,
            store: store,
        }
    }

    pub fn rate_limit(&self,
                      key: &str,
                      quantity: i64)
                      -> Result<(bool, RateLimitResult), ThrottleError> {
        println!("rate_limit: {} {}", key, quantity);
        Ok((false,
            RateLimitResult {
            limit: 0,
            remaining: 0,
            reset_after: time::Duration::seconds(1),
            retry_after: time::Duration::seconds(1),
        }))
    }
}

pub struct RateQuota {
    pub max_burst: i64,
    pub max_rate: Rate,
}

fn div_durations(x: time::Duration, y: time::Duration) -> time::Duration {
    time::Duration::nanoseconds(x.num_nanoseconds().unwrap() / y.num_nanoseconds().unwrap())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
