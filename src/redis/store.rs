extern crate time;

use error::ThrottleError;
use redis;
use throttle::store;

pub struct RedisStore<'a> {
    r: &'a redis::Redis,
}

impl<'a> RedisStore<'a> {
    pub fn new(r: &'a redis::Redis) -> RedisStore<'a> {
        RedisStore { r: r }
    }
}

impl<'a> store::Store for RedisStore<'a> {
    fn compare_and_swap_with_ttl(&self,
                                 key: &str,
                                 old: i64,
                                 new: i64,
                                 ttl: i64)
                                 -> Result<bool, ThrottleError> {
        let val = try!(self.r.get(key));
        match val {
            // Key did not exist.
            redis::Reply::Nil => Ok(false),

            // Still the old value.
            redis::Reply::Integer(n) if n == old => Ok(false),

            // Not the old value: perform the swap.
            redis::Reply::Integer(_) => {
                if ttl > 0 {
                    try!(self.r.setex(key, ttl, new.to_string().as_str()));
                } else {
                    try!(self.r.set(key, new.to_string().as_str()));
                }

                Ok(true)
            }

            _ => Err(ThrottleError::generic("GET returned non-string non-nil value.")),
        }
    }

    fn get_with_time(&self, key: &str) -> Result<(i64, time::Tm), ThrottleError> {
        // TODO: currently leveraging that CommandError and ThrottleError are the
        // same thing, but we should probably reconcile this.
        let val = try!(self.r.get(key));
        match val {
            redis::Reply::Integer(n) => Ok((n, time::now())),
            _ => Err(ThrottleError::generic(format!("Found non-integer in key: {}", key).as_str())),
        }
    }

    fn set_if_not_exists_with_ttl(&self,
                                  key: &str,
                                  value: i64,
                                  ttl: i64)
                                  -> Result<bool, ThrottleError> {
        let val = try!(self.r.setnx(key, value.to_string().as_str()));
        if ttl > 0 {
            try!(self.r.expire(key, ttl));
        }
        Ok(val)
    }
}
