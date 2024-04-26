# redis-cell [![Build Status](https://github.com/brandur/redis-cell/workflows/redis-cell%20CI/badge.svg)](https://github.com/brandur/redis-cell/actions)

**Warning:** This package is in "best effort" maintenance mode. I try to
respond to opened issues and keep it reasonably up-to-date with respect to the
underlying Rust toolchain, but am no longer actively developing it.

A Redis module that provides rate limiting in Redis as a single command.
Implements the fairly sophisticated [generic cell rate algorithm][gcra] (GCRA)
which provides a rolling time window and doesn't depend on a background drip
process.

The primitives exposed by Redis are perfect for doing work around rate
limiting, but because it's not built in, it's very common for companies and
organizations to implement their own rate limiting logic on top of Redis using
a mixture of basic commands and Lua scripts (I've seen this at both Heroku and
Stripe for example). This can often result in naive implementations that take a
few tries to get right. The directive of redis-cell is to provide a
language-agnostic rate limiter that's easily pluggable into many cloud
architectures.

[Informal benchmarks][benchmarks] show that redis-cell is pretty fast, taking a
little under twice as long to run as a basic Redis `SET` (very roughly 0.1 ms
per command as seen from a Redis client).

## Install

[Binaries for redis-cell are available for Mac and Linux][releases]. Open
an issue if there's interest in having binaries for architectures or operating
systems that are not currently supported.

Download and extract the library, then move it somewhere that Redis can access
it (note that the extension will be **.dylib** instead of **.so** for Mac
releases):

```
$ tar -zxf redis-cell-*.tar.gz
$ cp libredis_cell.so /path/to/modules/
```

**Or**, clone and build the project from source. You'll need to [install
Rust][rust-downloads] to do so (this may be as easy as a `brew install rust` if
you're on Mac).

```
$ git clone https://github.com/brandur/redis-cell.git
$ cd redis-cell
$ cargo build --release
$ cp target/release/libredis_cell.dylib /path/to/modules/
```

**Note that Rust 1.13.0+ is required.**

Run Redis pointing to the newly built module:

```
redis-server --loadmodule /path/to/modules/libredis_cell.so
```

Alternatively add the following to a `redis.conf` file:

```
loadmodule /path/to/modules/libredis_cell.so
```

## Usage

From Redis (try running `redis-cli`) use the new `CL.THROTTLE` command loaded by
the module. It's used like this:

```
CL.THROTTLE <key> <max_burst> <count per period> <period> [<quantity>]
```

Where `key` is an identifier to rate limit against. Examples might be:

* A user account's unique identifier.
* The origin IP address of an incoming request.
* A static string (e.g. `global`) to limit actions across the entire system.

For example:

```
CL.THROTTLE user123 15 30 60 1
               ▲     ▲  ▲  ▲ ▲
               |     |  |  | └───── apply 1 token (default if omitted)
               |     |  └──┴─────── 30 tokens / 60 seconds
               |     └───────────── 15 max_burst
               └─────────────────── key "user123"
```

### Response

This means that a single token (the `1` in the last parameter) should be
applied against the rate limit of the key `user123`. 30 tokens on the key are
allowed over a 60 second period with a maximum initial burst of 15 tokens. Rate
limiting parameters are provided with every invocation so that limits can
easily be reconfigured on the fly.

The command will respond with an array of integers:

```
127.0.0.1:6379> CL.THROTTLE user123 15 30 60
1) (integer) 0
2) (integer) 16
3) (integer) 15
4) (integer) -1
5) (integer) 2
```

The meaning of each array item is:

1. Whether the action was limited:
    * `0` indicates the action is allowed.
    * `1` indicates that the action was limited/blocked.
2. The total limit of the key (`max_burst` + 1). This is equivalent to the
   common `X-RateLimit-Limit` HTTP header.
3. The remaining limit of the key. Equivalent to `X-RateLimit-Remaining`.
4. The number of seconds until the user should retry, and always `-1` if the
   action was allowed. Equivalent to `Retry-After`.
5. The number of seconds until the limit will reset to its maximum capacity.
   Equivalent to `X-RateLimit-Reset`.

### Multiple Rate Limits

Implement different types of rate limiting by using different key names:

```
CL.THROTTLE user123-read-rate 15 30 60
CL.THROTTLE user123-write-rate 5 10 60
```

## On Rust

redis-cell is written in Rust and uses the language's FFI module to interact
with [Redis' own module system][redis-modules]. Rust makes a very good fit here
because it doesn't need a GC and is bootstrapped with only a tiny runtime.

The author of this library is of the opinion that writing modules in Rust
instead of C will convey similar performance characteristics, but result in an
implementation that's more likely to be devoid of the bugs and memory pitfalls
commonly found in many C programs.

## License

This is free software under the terms of MIT the license (see the file
`LICENSE` for details).

## Development

### Tests and checks

Run the test suite:

    cargo test

    # specific test
    cargo test it_rates_limits

    # with debug output on stdout
    cargo test it_rates_limits -- --nocapture

CI has checks for both [Rustfmt][rustfmt] and [Clippy][clippy] (Rust's linter).
These can be installed and run locally using Rustup's component framework:

    rustup component add rustfmt
    cargo fmt --all

    rustup component add clippy
    cargo clippy -- -D warnings

### Releasing

Releases are performed automatically from a script in CI which activates when a
new tag of the format `v1.2.3` is released. The script builds binaries for all
target systems and uploads them to GitHub's [releases][releases] page.

To perform a release:

1. Add a changelog entry in `CHANGELOG.md` using the existing format.
2. Bump the version number in `Cargo.toml`.
3. Commit these changes with a message like `Bump to version 1.2.3`.
4. Tag the release with `git tag v1.2.3` (make sure to include a leading `v`).
5. `ggpush --tags`
6. Edit the new release's title and body in [GitHub][releases] (a human touch
   is still expected for the final product). Use the contents for the new
   version from `CHANGELOG.md` as the release's body, which allows Markdown
   content.

[benchmarks]: https://gist.github.com/brandur/90698498bd543598d00df46e32be3268
[clippy]: https://github.com/rust-lang/rust-clippy
[gcra]: https://en.wikipedia.org/wiki/Generic_cell_rate_algorithm
[redis-modules]: https://github.com/redis/redis-doc/blob/master/docs/reference/modules/_index.md
[releases]: https://github.com/brandur/redis-cell/releases
[rust-downloads]: https://www.rust-lang.org/en-US/downloads.html
[rustfmt]: https://github.com/rust-lang/rustfmt

<!--
# vim: set tw=79:
-->
