extern crate time;

use error::ThrottleError;

pub trait Store {
    fn compare_and_swap_with_ttl(&self,
                                 key: &str,
                                 old: i64,
                                 new: i64,
                                 ttl: i64)
                                 -> Result<bool, ThrottleError>;

    fn get_with_time(&self, key: &str) -> Result<(i64, time::Tm), ThrottleError>;

    fn set_if_not_exists_with_ttl(&self,
                                  key: &str,
                                  value: i64,
                                  ttl: i64)
                                  -> Result<bool, ThrottleError>;
}
