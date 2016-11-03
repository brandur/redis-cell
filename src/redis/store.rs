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
        let val = try!(self.r.coerce_integer(self.r.get(key)));
        match val {
            redis::Reply::Nil => Ok(false),

            // Still the old value: perform the swap.
            redis::Reply::Integer(n) if n == old => {
                if ttl.num_seconds() > 1 {
                    try!(self.r.setex(key, ttl.num_seconds(), new.to_string().as_str()));
                } else {
                    try!(self.r.set(key, new.to_string().as_str()));
                }

                Ok(true)
            }

            // Not the old value: something else must have set it. Take no
            // action.
            redis::Reply::Integer(_) => Ok(false),

            _ => Err(ThrottleError::generic("GET returned non-string non-nil value.")),
        }
    }

    fn get_with_time(&self, key: &str) -> Result<(i64, time::Tm), ThrottleError> {
        // TODO: currently leveraging that CommandError and ThrottleError are the
        // same thing, but we should probably reconcile this.
        let val = try!(self.r.coerce_integer(self.r.get(key)));
        match val {
            redis::Reply::Nil => Ok((-1, time::now_utc())),

            redis::Reply::Integer(n) => Ok((n, time::now_utc())),

            x => {
                Err(ThrottleError::generic(format!("Found non-integer in key: {} (type: {:?})",
                                                   key,
                                                   x)
                    .as_str()))
            }
        }
    }

    fn log_debug(&self, message: &str) {
        self.r.log_debug(message);
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
