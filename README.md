# redis-throttle

Provides basic rate limiting from within Redis as a module.

## Build and Run Instructions

Build the project:

```
cargo build
```

In `redis.conf`:

```
loadmodule target/debug/libredis_throttle.dylib
```

Then run Redis:

```
$ redis-server redis.conf
```
