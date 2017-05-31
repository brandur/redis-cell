extern crate libc;
extern crate time;

#[macro_use]
extern crate bitflags;

#[allow(dead_code)]
pub mod redis;
pub mod error;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
