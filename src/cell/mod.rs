extern crate time;

pub mod store;

use error::CellError;

// Maximum number of times to retry set_if_not_exists/compare_and_swap
// operations before returning an error.
const MAX_CAS_ATTEMPTS: i64 = 5;

#[derive(Debug, PartialEq)]
pub struct Rate {
    pub period: time::Duration,
}

impl Rate {
    pub fn per_day(n: i64) -> Rate {
        Rate::per_period(n, time::Duration::days(1))
    }

    pub fn per_hour(n: i64) -> Rate {
        Rate::per_period(n, time::Duration::hours(1))
    }

    pub fn per_minute(n: i64) -> Rate {
        Rate::per_period(n, time::Duration::minutes(1))
    }

    /// Produces a rate for some number of actions per second. For example, if
    /// we wanted to have 10 actions every 2 seconds, the period produced would
    /// be 200 ms.
    pub fn per_period(n: i64, period: time::Duration) -> Rate {
        let ns: i64 = period.num_nanoseconds().unwrap();
        let period = time::Duration::nanoseconds(((ns as f64) / (n as f64)) as i64);
        Rate { period }
    }

    pub fn per_second(n: i64) -> Rate {
        Rate::per_period(n, time::Duration::seconds(1))
    }
}

#[derive(Debug, PartialEq)]
pub struct RateLimitResult {
    pub limit:       i64,
    pub remaining:   i64,
    pub reset_after: time::Duration,
    pub retry_after: time::Duration,
}

pub struct RateLimiter<'a, T: 'a + store::Store> {
    pub store: &'a mut T,

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

impl<'a, T: 'a + store::Store> RateLimiter<'a, T> {
    pub fn new(store: &'a mut T, quota: &RateQuota) -> RateLimiter<'a, T> {
        RateLimiter {
            delay_variation_tolerance: time::Duration::nanoseconds(
                quota.max_rate.period.num_nanoseconds().unwrap() * (quota.max_burst + 1),
            ),
            emission_interval: quota.max_rate.period,
            limit: quota.max_burst + 1,
            store,
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
    pub fn rate_limit(
        &mut self,
        key: &str,
        quantity: i64,
    ) -> Result<(bool, RateLimitResult), CellError> {
        let mut rlc = RateLimitResult {
            limit:       self.limit,
            remaining:   0,
            retry_after: time::Duration::seconds(-1),
            reset_after: time::Duration::seconds(-1),
        };

        let increment = time::Duration::nanoseconds(
            self.emission_interval.num_nanoseconds().unwrap() * quantity,
        );
        self.log_start(key, quantity, increment);

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
        // normal case for the redis-cell project) this is actually *not* true
        // because our entire operation will execute atomically.
        let mut i = 0;
        loop {
            log_debug!(self.store, "iteration = {}", i);

            // tat refers to the theoretical arrival time that would be expected
            // from equally spaced requests at exactly the rate limit.
            let (tat_val, now) = self.store.get_with_time(key)?;

            let tat = match tat_val {
                -1 => now,
                _ => from_nanoseconds(tat_val),
            };
            log_debug!(
                self.store,
                "tat = {} (from store = {})",
                tat.rfc3339(),
                tat_val
            );

            let new_tat = if now > tat {
                now + increment
            } else {
                tat + increment
            };
            log_debug!(self.store, "new_tat = {}", new_tat.rfc3339());

            // Block the request if the next permitted time is in the future.
            let allow_at = new_tat - self.delay_variation_tolerance;
            let diff = now - allow_at;
            log_debug!(
                self.store,
                "diff = {}ms (now - allow_at)",
                diff.num_milliseconds()
            );

            if diff < time::Duration::zero() {
                log_debug!(
                    self.store,
                    "BLOCKED retry_after = {}ms",
                    -diff.num_milliseconds()
                );

                if increment <= self.delay_variation_tolerance {
                    rlc.retry_after = -diff;
                }

                limited = true;
                ttl = tat - now;
                break;
            }

            let new_tat_ns = nanoseconds(new_tat);
            ttl = new_tat - now;
            log_debug!(self.store, "ALLOWED");

            // If the key was originally missing, set it if if doesn't exist.
            // If it was there, try to compare and swap.
            //
            // Both of these cases are designed to work around the fact that
            // another limiter could be running in parallel.
            let updated = if tat_val == -1 {
                self.store.set_if_not_exists_with_ttl(key, new_tat_ns, ttl)?
            } else {
                self.store
                    .compare_and_swap_with_ttl(key, tat_val, new_tat_ns, ttl)?
            };

            if updated {
                limited = false;
                break;
            }

            i += 1;
            if i > MAX_CAS_ATTEMPTS {
                return Err(error!(
                    "Failed to update rate limit after \
                     {} attempts",
                    MAX_CAS_ATTEMPTS
                ));
            }
        }

        let next = self.delay_variation_tolerance - ttl;
        if next > -self.emission_interval {
            rlc.remaining = (next.num_microseconds().unwrap() as f64
                / self.emission_interval.num_microseconds().unwrap() as f64)
                as i64;
        }
        rlc.reset_after = ttl;

        self.log_end(&rlc);
        Ok((limited, rlc))
    }

    fn log_end(&self, rlc: &RateLimitResult) {
        log_debug!(
            self.store,
            "limit = {} remaining = {}",
            self.limit,
            rlc.remaining
        );
        log_debug!(
            self.store,
            "retry_after = {}ms",
            rlc.retry_after.num_milliseconds()
        );
        log_debug!(
            self.store,
            "reset_after = {}ms (ttl)",
            rlc.reset_after.num_milliseconds()
        );
    }

    fn log_start(&self, key: &str, quantity: i64, increment: time::Duration) {
        log_debug!(self.store, "");
        log_debug!(self.store, "-----");
        log_debug!(self.store, "key = {}", key);
        log_debug!(self.store, "quantity = {}", quantity);
        log_debug!(
            self.store,
            "delay_variation_tolerance = {}ms",
            self.delay_variation_tolerance.num_milliseconds()
        );
        log_debug!(
            self.store,
            "emission_interval = {}ms",
            self.emission_interval.num_milliseconds()
        );
        log_debug!(
            self.store,
            "tat_increment = {}ms (emission_interval * quantity)",
            increment.num_milliseconds()
        );
    }
}

#[derive(Debug, PartialEq)]
pub struct RateQuota {
    pub max_burst: i64,
    pub max_rate:  Rate,
}

fn from_nanoseconds(x: i64) -> time::Tm {
    let ns = (10 as i64).pow(9);
    time::at(time::Timespec {
        sec:  x / ns,
        nsec: (x % ns) as i32,
    })
}

fn nanoseconds(x: time::Tm) -> i64 {
    let ts = x.to_timespec();
    ts.sec * (10 as i64).pow(9) + i64::from(ts.nsec)
}

#[cfg(test)]
mod tests {
    extern crate time;

    use cell::*;
    use error::CellError;
    use std::error::Error;

    #[test]
    fn it_creates_rates_from_days() {
        assert_eq!(
            Rate {
                period: time::Duration::hours(1),
            },
            Rate::per_day(24)
        )
    }

    #[test]
    fn it_creates_rates_from_hours() {
        assert_eq!(
            Rate {
                period: time::Duration::minutes(10),
            },
            Rate::per_hour(6)
        )
    }

    #[test]
    fn it_creates_rates_from_minutes() {
        assert_eq!(
            Rate {
                period: time::Duration::seconds(10),
            },
            Rate::per_minute(6)
        )
    }

    #[test]
    fn it_creates_rates_from_periods() {
        assert_eq!(
            Rate {
                period: time::Duration::seconds(20),
            },
            Rate::per_period(6, time::Duration::minutes(2))
        )
    }

    #[test]
    fn it_creates_rates_from_seconds() {
        assert_eq!(
            Rate {
                period: time::Duration::milliseconds(200),
            },
            Rate::per_second(5)
        )
    }

    // Skip rustfmt so we don't mangle our big test case array below which is
    // already hard enough to read.
    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[test]
    fn it_rate_limits() {
        let limit = 5;
        let quota = RateQuota {
            max_burst: limit - 1,
            max_rate: Rate::per_second(1),
        };
        let start = time::now_utc();
        let mut memory_store = store::MemoryStore::new_verbose();
        let mut test_store = TestStore::new(&mut memory_store);
        let mut limiter = RateLimiter::new(&mut test_store, &quota);

        let cases = [
            //
            // (test case #, now, volume, remaining, reset_after, retry_after, limited)
            //

            // You can never make a request larger than the maximum.
            RateLimitCase::new(0, start, 6, 5, time::Duration::zero(),
                time::Duration::seconds(-1), true),

            // Rate limit normal requests appropriately.
            RateLimitCase::new(1, start, 1, 4, time::Duration::seconds(1),
                time::Duration::seconds(-1), false),
            RateLimitCase::new(2, start, 1, 3, time::Duration::seconds(2),
                time::Duration::seconds(-1), false),
            RateLimitCase::new(3, start, 1, 2, time::Duration::seconds(3),
                time::Duration::seconds(-1), false),
            RateLimitCase::new(4, start, 1, 1, time::Duration::seconds(4),
                time::Duration::seconds(-1), false),
            RateLimitCase::new(5, start, 1, 0, time::Duration::seconds(5),
                time::Duration::seconds(-1), false),
            RateLimitCase::new(6, start, 1, 0, time::Duration::seconds(5),
                time::Duration::seconds(1), true),

            RateLimitCase::new(7, start + time::Duration::milliseconds(3000), 1, 2,
                time::Duration::milliseconds(3000), time::Duration::seconds(-1), false),
            RateLimitCase::new(8, start + time::Duration::milliseconds(3100), 1, 1,
                time::Duration::milliseconds(3900), time::Duration::seconds(-1), false),
            RateLimitCase::new(9, start + time::Duration::milliseconds(4000), 1, 1,
                time::Duration::milliseconds(4000), time::Duration::seconds(-1), false),
            RateLimitCase::new(10, start + time::Duration::milliseconds(8000), 1, 4,
                time::Duration::milliseconds(1000), time::Duration::seconds(-1), false),
            RateLimitCase::new(11, start + time::Duration::milliseconds(9500), 1, 4,
                time::Duration::milliseconds(1000), time::Duration::seconds(-1), false),

            // Zero-volume request just peeks at the state.
            RateLimitCase::new(12, start + time::Duration::milliseconds(9500), 0, 4,
                time::Duration::seconds(1), time::Duration::seconds(-1), false),

            // High-volume request uses up more of the limit.
            RateLimitCase::new(13, start + time::Duration::milliseconds(9500), 2, 2,
                time::Duration::seconds(3), time::Duration::seconds(-1), false),

            // Large requests cannot exceed limits
            RateLimitCase::new(14, start + time::Duration::milliseconds(9500), 5, 2,
                time::Duration::seconds(3), time::Duration::seconds(3), true),
        ];

        for case in cases.iter() {
            println!("starting test case = {:?}", case.num);
            println!("{:?}", case);

            limiter.store.clock = case.now;
            let (limited, results) = limiter.rate_limit("foo", case.volume).unwrap();

            println!("limited = {:?}", limited);
            println!("{:?}", results);
            println!("");

            assert_eq!(case.limited, limited);
            assert_eq!(limit, results.limit);
            assert_eq!(case.remaining, results.remaining);
            assert_eq!(case.reset_after, results.reset_after);
            assert_eq!(case.retry_after, results.retry_after);
        }
    }

    #[test]
    fn it_handles_rate_limit_update_failures() {
        let quota = RateQuota {
            max_burst: 1,
            max_rate:  Rate::per_second(1),
        };
        let mut memory_store = store::MemoryStore::new_verbose();
        let mut test_store = TestStore::new(&mut memory_store);
        test_store.fail_updates = true;

        let mut limiter = RateLimiter::new(&mut test_store, &quota);

        let err = error!("Failed to update rate limit after 5 attempts");

        assert_eq!(
            err.description(),
            limiter.rate_limit("foo", 1).unwrap_err().description()
        );
    }

    #[derive(Debug, PartialEq)]
    struct RateLimitCase {
        num:         i64,
        now:         time::Tm,
        volume:      i64,
        remaining:   i64,
        reset_after: time::Duration,
        retry_after: time::Duration,
        limited:     bool,
    }

    impl RateLimitCase {
        fn new(
            num: i64,
            now: time::Tm,
            volume: i64,
            remaining: i64,
            reset_after: time::Duration,
            retry_after: time::Duration,
            limited: bool,
        ) -> RateLimitCase {
            return RateLimitCase {
                num:         num,
                now:         now,
                volume:      volume,
                remaining:   remaining,
                reset_after: reset_after,
                retry_after: retry_after,
                limited:     limited,
            };
        }
    }

    /// TestStore is a Store implementation that wraps a MemoryStore and allows
    /// us to tweak certain behavior, like for example setting the effective
    /// system clock.
    struct TestStore<'a> {
        clock:        time::Tm,
        fail_updates: bool,
        store:        &'a mut store::MemoryStore,
    }

    impl<'a> TestStore<'a> {
        fn new(store: &'a mut store::MemoryStore) -> TestStore {
            TestStore {
                clock:        time::empty_tm(),
                fail_updates: false,
                store:        store,
            }
        }
    }

    impl<'a> store::Store for TestStore<'a> {
        fn compare_and_swap_with_ttl(
            &mut self,
            key: &str,
            old: i64,
            new: i64,
            ttl: time::Duration,
        ) -> Result<bool, CellError> {
            if self.fail_updates {
                Ok(false)
            } else {
                self.store.compare_and_swap_with_ttl(key, old, new, ttl)
            }
        }

        fn get_with_time(&self, key: &str) -> Result<(i64, time::Tm), CellError> {
            let tup = self.store.get_with_time(key)?;
            Ok((tup.0, self.clock))
        }

        fn log_debug(&self, message: &str) {
            self.store.log_debug(message)
        }

        fn set_if_not_exists_with_ttl(
            &mut self,
            key: &str,
            value: i64,
            ttl: time::Duration,
        ) -> Result<bool, CellError> {
            if self.fail_updates {
                Ok(false)
            } else {
                self.store.set_if_not_exists_with_ttl(key, value, ttl)
            }
        }
    }
}
