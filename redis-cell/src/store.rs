use crate::redis;
use redis_cell_impl::time::{Duration, OffsetDateTime};
use redis_cell_impl::{CellError, Store};

/// `InternalRedisStore` is a store implementation that uses Redis module APIs
/// in that it's designed to run from within a Redis runtime. This allows us to
/// cut some corners around atomicity because we can safety assume that all
/// operations will be atomic.
pub struct InternalRedisStore<'a> {
    r: &'a redis::Redis,
}

impl<'a> InternalRedisStore<'a> {
    pub fn new(r: &'a redis::Redis) -> InternalRedisStore<'a> {
        InternalRedisStore { r }
    }
}

impl<'a> Store for InternalRedisStore<'a> {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: Duration,
    ) -> Result<bool, CellError> {
        let key = self.r.open_key_writable(key);
        match key.read()? {
            Some(s) => {
                // While we will usually have a value here to parse, it's possible that
                // in the case of a very fast rate the key's already been
                // expired even since the beginning of this operation.
                // Check whether the value is empty to handle that possibility.
                if !s.is_empty() && s.parse::<i64>()? == old {
                    // Still the old value: perform the swap.
                    key.write(new.to_string().as_str())?;
                    key.set_expire(ttl)?;
                    Ok(true)
                } else {
                    // Not the old value: something else must have set it. Take no
                    // action.
                    Ok(false)
                }
            }

            // Value wasn't set.
            None => Ok(false),
        }
    }

    fn get_with_time(&self, key: &str) -> Result<(i64, OffsetDateTime), CellError> {
        // TODO: currently leveraging that CommandError and CellError are the
        // same thing, but we should probably reconcile this.
        let key = self.r.open_key(key);
        match key.read()? {
            Some(s) => {
                let n = s.parse::<i64>()?;
                Ok((n, OffsetDateTime::now_utc()))
            }
            None => Ok((-1, OffsetDateTime::now_utc())),
        }
    }

    fn log_debug(&self, message: &str) {
        self.r.log_debug(message);
    }

    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: Duration,
    ) -> Result<bool, CellError> {
        let key = self.r.open_key_writable(key);
        let res = if key.is_empty()? {
            key.write(value.to_string().as_str())?;
            Ok(true)
        } else {
            Ok(false)
        };
        key.set_expire(ttl)?;
        res
    }
}
