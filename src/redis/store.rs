extern crate time;

use throttle::store;

pub struct RedisStore<'a> {
    db: i64,
    prefix: &'a str,
}

impl<'a> RedisStore<'a> {
    pub fn new(db: i64) -> RedisStore<'a> {
        RedisStore {
            prefix: "",
            db: db,
        }
    }

    pub fn new_with_prefix(prefix: &str, db: i64) -> RedisStore {
        RedisStore {
            prefix: prefix,
            db: db,
        }
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

    fn get_with_time(&self, key: &str) -> Result<(i64, time::Tm), store::StoreError> {}

    fn set_if_not_exists_with_ttl(&self,
                                  key: &str,
                                  value: i64,
                                  ttl: time::Tm)
                                  -> Result<bool, store::StoreError> {
        Result::Err(store::StoreError::new("not implemented"))
    }
}
