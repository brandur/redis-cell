extern crate time;

use error::ThrottleError;
use redis;
use std::collections::HashMap;

/// Store exposes the atomic data store operations that the GCRA rate limiter
/// needs to function correctly.
///
/// Note that because the default mode for this library is to run within Redis
/// (making every operation atomic by default), the encapsulation is not
/// strictly needed. However, it's written to be generic enough that a
/// out-of-Redis Store could be written and have the rate limiter still work
/// properly.
pub trait Store {
    /// Compares the value at the given key with a known old value and swaps it
    /// for a new value if and only if they're equal. Also sets the key's TTL
    /// until it expires.
    fn compare_and_swap_with_ttl(&mut self,
                                 key: &str,
                                 old: i64,
                                 new: i64,
                                 ttl: time::Duration)
                                 -> Result<bool, ThrottleError>;

    /// Gets the given key's value and the current time as dictated by the
    /// store (this is done so that rate limiters running on a variety of
    /// different nodes can operate with a consistent clock instead of using
    /// their own). If the key was unset, -1 is returned.
    fn get_with_time(&self,
                     key: &str)
                     -> Result<(i64, time::Tm), ThrottleError>;

    /// Logs a debug message to the data store.
    fn log_debug(&self, message: &str);

    /// Sets the given key to the given value if and only if it doesn't already
    /// exit. Whether or not the key existed previously it's given a new TTL.
    fn set_if_not_exists_with_ttl(&mut self,
                                  key: &str,
                                  value: i64,
                                  ttl: time::Duration)
                                  -> Result<bool, ThrottleError>;
}

/// MemoryStore is a simple implementation of Store that persists data in an
/// in-memory HashMap.
///
/// Note that the implementation is currently not thread-safe and will need a
/// mutex added if it's ever used for anything serious.
pub struct MemoryStore {
    map: HashMap<String, i64>,
    verbose: bool,
}

impl MemoryStore {
    pub fn new() -> MemoryStore {
        MemoryStore {
            map: HashMap::new(),
            verbose: false,
        }
    }

    pub fn new_verbose() -> MemoryStore {
        MemoryStore {
            map: HashMap::new(),
            verbose: true,
        }
    }
}

impl Store for MemoryStore {
    fn compare_and_swap_with_ttl(&mut self,
                                 key: &str,
                                 old: i64,
                                 new: i64,
                                 _: time::Duration)
                                 -> Result<bool, ThrottleError> {
        match self.map.get(key) {
            Some(n) if *n != old => return Ok(false),
            _ => (),
        };

        self.map.insert(String::from(key), new);
        Ok(true)
    }

    fn get_with_time(&self,
                     key: &str)
                     -> Result<(i64, time::Tm), ThrottleError> {
        match self.map.get(key) {
            Some(n) => Ok((*n, time::now_utc())),
            None => Ok((-1, time::now_utc())),
        }
    }

    fn log_debug(&self, message: &str) {
        if self.verbose {
            println!("memory_store: {}", message);
        }
    }

    fn set_if_not_exists_with_ttl(&mut self,
                                  key: &str,
                                  value: i64,
                                  _: time::Duration)
                                  -> Result<bool, ThrottleError> {
        match self.map.get(key) {
            Some(_) => Ok(false),
            None => {
                self.map.insert(String::from(key), value);
                Ok(true)
            }
        }
    }
}

/// InternalRedisStore is a store implementation that uses Redis module APIs in
/// that it's designed to run from within a Redis runtime. This allows us to
/// cut some corners around atomicity because we can safety assume that all
/// operations will be atomic.
pub struct InternalRedisStore<'a> {
    r: &'a redis::Redis,
}

impl<'a> InternalRedisStore<'a> {
    pub fn new(r: &'a redis::Redis) -> InternalRedisStore<'a> {
        InternalRedisStore { r: r }
    }
}

impl<'a> Store for InternalRedisStore<'a> {
    fn compare_and_swap_with_ttl(&mut self,
                                 key: &str,
                                 old: i64,
                                 new: i64,
                                 ttl: time::Duration)
                                 -> Result<bool, ThrottleError> {
        let val = try!(self.r.coerce_integer(self.r.get(key)));
        match val {
            redis::Reply::Nil => Ok(false),

            // Still the old value: perform the swap.
            redis::Reply::Integer(n) if n == old => {
                if ttl.num_seconds() > 1 {
                    try!(self.r.setex(key,
                                      ttl.num_seconds(),
                                      new.to_string().as_str()));
                } else {
                    try!(self.r.set(key, new.to_string().as_str()));
                }

                Ok(true)
            }

            // Not the old value: something else must have set it. Take no
            // action.
            redis::Reply::Integer(_) => Ok(false),

            _ => Err(error!("GET returned non-string non-nil value.")),
        }
    }

    fn get_with_time(&self,
                     key: &str)
                     -> Result<(i64, time::Tm), ThrottleError> {
        // TODO: currently leveraging that CommandError and ThrottleError are the
        // same thing, but we should probably reconcile this.
        let val = try!(self.r.coerce_integer(self.r.get(key)));
        match val {
            redis::Reply::Nil => Ok((-1, time::now_utc())),
            redis::Reply::Integer(n) => Ok((n, time::now_utc())),
            x => {
                Err(error!("Found non-integer in key: {} (type: {:?})",
                           key,
                           x))
            }
        }
    }

    fn log_debug(&self, message: &str) {
        self.r.log_debug(message);
    }

    fn set_if_not_exists_with_ttl(&mut self,
                                  key: &str,
                                  value: i64,
                                  ttl: time::Duration)
                                  -> Result<bool, ThrottleError> {
        let val = try!(self.r.setnx(key, value.to_string().as_str()));
        if ttl.num_seconds() > 1 {
            try!(self.r.expire(key, ttl.num_seconds()));
        }
        Ok(val)
    }
}


#[cfg(test)]
mod tests {
    extern crate time;

    use throttle::store::*;

    #[test]
    fn it_performs_compare_and_swap_with_ttl() {
        let mut store = MemoryStore::new();

        // First attempt obviously works.
        let res1 = store.compare_and_swap_with_ttl("foo",
                                                   123,
                                                   124,
                                                   time::Duration::zero());
        assert_eq!(true, res1.unwrap());

        // Second attempt succeeds: we use the value we just set combined with
        // a new value.
        let res2 = store.compare_and_swap_with_ttl("foo",
                                                   124,
                                                   125,
                                                   time::Duration::zero());
        assert_eq!(true, res2.unwrap());

        // Third attempt fails: we try to overwrite using a value that is
        // incorrect.
        let res2 = store.compare_and_swap_with_ttl("foo",
                                                   123,
                                                   126,
                                                   time::Duration::zero());
        assert_eq!(false, res2.unwrap());
    }

    #[test]
    fn it_performs_get_with_time() {
        let mut store = MemoryStore::new();

        let res1 = store.get_with_time("foo");
        assert_eq!(-1, res1.unwrap().0);

        // Now try setting a value.
        let _ =
            store.set_if_not_exists_with_ttl("foo",
                                            123,
                                            time::Duration::zero())
                .unwrap();

        let res2 = store.get_with_time("foo");
        assert_eq!(123, res2.unwrap().0);
    }

    #[test]
    fn it_performs_set_if_not_exists_with_ttl() {
        let mut store = MemoryStore::new();

        let res1 =
            store.set_if_not_exists_with_ttl("foo",
                                             123,
                                             time::Duration::zero());
        assert_eq!(true, res1.unwrap());

        let res2 =
            store.set_if_not_exists_with_ttl("foo",
                                             123,
                                             time::Duration::zero());
        assert_eq!(false, res2.unwrap());
    }
}
