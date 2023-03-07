use crate::error::CellError;
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
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: time::Duration,
    ) -> Result<bool, CellError>;

    /// Gets the given key's value and the current time as dictated by the
    /// store (this is done so that rate limiters running on a variety of
    /// different nodes can operate with a consistent clock instead of using
    /// their own). If the key was unset, -1 is returned.
    fn get_with_time(&self, key: &str) -> Result<(i64, time::Tm), CellError>;

    /// Logs a debug message to the data store.
    fn log_debug(&self, message: &str);

    /// Sets the given key to the given value if and only if it doesn't already
    /// exit. Whether or not the key existed previously it's given a new TTL.
    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: time::Duration,
    ) -> Result<bool, CellError>;
}

// Implement the `Store` trait for a mutable reference. This is useful so that
// we don't have to assign a lifetime (`'a`) to `RateLimiter`, thus simplifying
// our code there by quite a bit.
impl<'a, T: Store> Store for &'a mut T {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        ttl: time::Duration,
    ) -> Result<bool, CellError> {
        (**self).compare_and_swap_with_ttl(key, old, new, ttl)
    }

    fn get_with_time(&self, key: &str) -> Result<(i64, time::Tm), CellError> {
        (**self).get_with_time(key)
    }

    fn log_debug(&self, message: &str) {
        (**self).log_debug(message)
    }

    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        ttl: time::Duration,
    ) -> Result<bool, CellError> {
        (**self).set_if_not_exists_with_ttl(key, value, ttl)
    }
}

/// `MemoryStore` is a simple implementation of Store that persists data in an
/// in-memory `HashMap`.
///
/// Note that the implementation is currently not thread-safe and will need a
/// mutex added if it's ever used for anything serious.
#[derive(Default)]
pub struct MemoryStore {
    map: HashMap<String, i64>,
    verbose: bool,
}

impl MemoryStore {
    pub fn new() -> MemoryStore {
        Self::default()
    }

    pub fn new_verbose() -> MemoryStore {
        MemoryStore {
            map: HashMap::new(),
            verbose: true,
        }
    }
}

impl Store for MemoryStore {
    fn compare_and_swap_with_ttl(
        &mut self,
        key: &str,
        old: i64,
        new: i64,
        _: time::Duration,
    ) -> Result<bool, CellError> {
        match self.map.get(key) {
            Some(n) if *n != old => return Ok(false),
            _ => (),
        };

        self.map.insert(String::from(key), new);
        Ok(true)
    }

    fn get_with_time(&self, key: &str) -> Result<(i64, time::Tm), CellError> {
        match self.map.get(key) {
            Some(n) => Ok((*n, time::now_utc())),
            None => Ok((-1, time::now_utc())),
        }
    }

    fn log_debug(&self, message: &str) {
        if self.verbose {
            println!("memory_store: {message}");
        }
    }

    fn set_if_not_exists_with_ttl(
        &mut self,
        key: &str,
        value: i64,
        _: time::Duration,
    ) -> Result<bool, CellError> {
        match self.map.get(key) {
            Some(_) => Ok(false),
            None => {
                self.map.insert(String::from(key), value);
                Ok(true)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_performs_compare_and_swap_with_ttl() {
        let mut store = MemoryStore::default();

        // First attempt obviously works.
        let res1 =
            store.compare_and_swap_with_ttl("foo", 123, 124, time::Duration::zero());
        assert_eq!(true, res1.unwrap());

        // Second attempt succeeds: we use the value we just set combined with
        // a new value.
        let res2 =
            store.compare_and_swap_with_ttl("foo", 124, 125, time::Duration::zero());
        assert_eq!(true, res2.unwrap());

        // Third attempt fails: we try to overwrite using a value that is
        // incorrect.
        let res2 =
            store.compare_and_swap_with_ttl("foo", 123, 126, time::Duration::zero());
        assert_eq!(false, res2.unwrap());
    }

    #[test]
    fn it_performs_get_with_time() {
        let mut store = MemoryStore::default();

        let res1 = store.get_with_time("foo");
        assert_eq!(-1, res1.unwrap().0);

        // Now try setting a value.
        let _ = store
            .set_if_not_exists_with_ttl("foo", 123, time::Duration::zero())
            .unwrap();

        let res2 = store.get_with_time("foo");
        assert_eq!(123, res2.unwrap().0);
    }

    #[test]
    fn it_performs_set_if_not_exists_with_ttl() {
        let mut store = MemoryStore::default();

        let res1 = store.set_if_not_exists_with_ttl("foo", 123, time::Duration::zero());
        assert_eq!(true, res1.unwrap());

        let res2 = store.set_if_not_exists_with_ttl("foo", 123, time::Duration::zero());
        assert_eq!(false, res2.unwrap());
    }
}
