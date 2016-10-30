extern crate time;

pub struct Rate {
    count: i64,
    period: time::Duration,
}

impl Rate {
    pub fn per_second(n: i64) -> Rate {
        Rate::per_time(n, |n: i64| -> time::Duration { time::Duration::seconds(n) })
    }

    pub fn per_time<F>(n: i64, make_duration: F) -> Rate
        where F : Fn(i64) -> time::Duration {
        Rate {
            count: n,
            period: div_durations(make_duration(1), make_duration(n))
        }
    }
}

pub struct RateQuota {
    max_burst: i64,
    max_rate: Rate,
}

fn div_durations(x: time::Duration, y: time::Duration) -> time::Duration {
    time::Duration::nanoseconds(x.num_nanoseconds().unwrap() / y.num_nanoseconds().unwrap())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
