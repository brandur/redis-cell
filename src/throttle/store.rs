extern crate time;

pub trait Store {
    fn compare_and_swap_with_ttl(&self, key: &str, old: i64, new: i64,
                                 ttl: time::Tm) -> bool;

    fn get_with_time(&self, key: &str) -> (i64, time::Tm);

    fn set_if_not_exists_with_ttl(&self, key: &str, value: i64,
                                  ttl: time::Tm) -> bool;
}
