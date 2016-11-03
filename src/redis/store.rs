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
                                 ttl: time::Duration)
                                 -> Result<bool, ThrottleError> {
        let val = try!(self.r.get(key));
        match val {
            // Key did not exist. The docs say that this should be a Redis nil,
            // but it apparently comes back as an unknown; just handle both.
            redis::Reply::Nil => Ok(false),
            redis::Reply::Unknown => Ok(false),

            // Still the old value.
            redis::Reply::Integer(n) if n == old => Ok(false),

            // Not the old value: perform the swap.
            redis::Reply::Integer(_) => {
                if ttl.num_seconds() > 1 {
                    try!(self.r.setex(key, ttl.num_seconds(), new.to_string().as_str()));
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
            // Key did not exist. The docs say that this should be a Redis nil,
            // but it apparently comes back as an unknown; just handle both.
            redis::Reply::Nil => Ok((0, time::now())),
            redis::Reply::Unknown => Ok((0, time::now())),

            redis::Reply::Integer(n) => Ok((n, time::now())),

            x => {
                Err(ThrottleError::generic(format!("Found non-integer in key: {} (type: {:?})",
                                                   key,
                                                   x)
                    .as_str()))
            }
        }
    }

    fn set_if_not_exists_with_ttl(&self,
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
