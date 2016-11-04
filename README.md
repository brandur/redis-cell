# redis-throttle [![Build Status](https://travis-ci.org/brandur/redis-throttle.svg?branch=master)](https://travis-ci.org/brandur/redis-throttle)

A Redis module that provides rate limiting in Redis as a single command.
Implements the fairly sophisticated [generic cell rate algorithm][gcra] (GCRA)
which provides a rolling time window and doesn't depend on a background drip
process.

The primitives exposed by Redis are perfect for doing work around rate
limiting, but because it's not built in, it's very common for companies and
organizations to implement their own rate limiting logic on top of Redis (I've
seen this at both Heroku and Stripe for example). This can often result in
naive implementations that take a few tries to get right. The directive of
redis-throttle is to provide a language-agnostic rate limiter that's easily
pluggable into many cloud architectures.

## Install

No binaries are currently being distributed, so it's necessary to build the
project from source. You'll need to [install Rust][rust-downloads] which may be
as easy as:

```
$ brew install rust
```

Clone and build the project:

```
$ git clone https://github.com/brandur/redis-throttle.git
$ cd redis-throttle
$ cargo build --release
```

Run Redis pointing to the newly built module:

```
redis-server --loadmodule target/release/libredis_throttle.dylib
```

Alternatively add the following to a `redis.conf` file:

```
loadmodule target/release/libredis_throttle.dylib
```

## Usage

From Redis (try running `redis-cli`) use the new `throttle` command loaded by
the module. It's used like this:

```
TH.THROTTLE <key> <max_burst> <count> <period> [<quantity>]
```

For example:

```
TH.THROTTLE user123 15 30 60
```

This means that a single action should be applied against the rate limit of the
bucket `user123`. 30 actions on the bucket are allowed over a 60 second period
with a maximum initial burst of 15 actions. Rate limiting parameters are
provided with every invocation so that buckets can easily be reconfigured on
the fly.

Implement multiple times of rate limiting by using different bucket names:

```
TH.THROTTLE user123-read-rate 15 30 60
TH.THROTTLE user123-write-rate 5 10 60
```

## On Rust

redis-throttle is written in Rust and uses the language's FFI module to
interact with [Redis' own module system][redis-modules]. Rust makes a very good
fit here because it doesn't need a GC or otherwise have any runtime of its own.

The author of this library is of the opinion that writing modules in Rust
instead of C will convey similar performance characteristics, but result in an
implementation that's more likely to be devoid of the bugs and memory pitfalls
commonly found in many C programs.

[gcra]: https://en.wikipedia.org/wiki/Generic_cell_rate_algorithm
[redis-modules]: https://github.com/antirez/redis/blob/unstable/src/modules/INTRO.md
[rust-downloads]: https://www.rust-lang.org/en-US/downloads.html
