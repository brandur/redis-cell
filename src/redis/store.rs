extern crate time;

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
                                 ttl: time::Tm)
                                 -> Result<bool, store::StoreError> {
        Result::Err(store::StoreError::new("not implemented"))
    }

    fn get_with_time(&self, key: &str) -> Result<(i64, time::Tm), store::StoreError> {
        Result::Err(store::StoreError::new("not implemented"))
    }

    fn set_if_not_exists_with_ttl(&self,
                                  key: &str,
                                  value: i64,
                                  ttl: time::Tm)
                                  -> Result<bool, store::StoreError> {
        Result::Err(store::StoreError::new("not implemented"))
    }
}
