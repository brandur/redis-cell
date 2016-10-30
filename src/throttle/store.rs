extern crate time;

use std::error;
use std::fmt;

pub trait Store {
    fn compare_and_swap_with_ttl(&self, key: &str, old: i64, new: i64,
                                 ttl: time::Tm) -> Result<bool, StoreError>;

    fn get_with_time(&self, key: &str) -> Result<(i64, time::Tm), StoreError>;

    fn set_if_not_exists_with_ttl(&self, key: &str, value: i64,
                                  ttl: time::Tm) -> Result<bool, StoreError>;
}

#[derive(Debug)]
pub struct StoreError<'a> {
    message: &'a str,
}

impl<'a> StoreError<'a> {
    pub fn new(message: &str) -> StoreError {
        StoreError{message: message}
    }
}

impl<'a> fmt::Display for StoreError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Store error: {}", self.message)
    }
}

impl<'a> error::Error for StoreError<'a> {
    fn description(&self) -> &str {
        self.message
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}
