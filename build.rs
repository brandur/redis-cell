extern crate cc;

fn main() {
    // Build a Redis (or Valkey) pseudo-library so that we have symbols that we
    // can link against while building Rust code.
    //
    // include/redismodule.h and include/valkeymodule.h are just vendored in from
    // the Redis and Valkey projects respectively and src/redismodule.c along with
    // src/valkeymodule.c are just stubs that include them and play a few other
    // tricks that we need to complete the build.
    let stub = if cfg!(feature = "valkey") {
        "src/valkeymodule.c"
    } else {
        "src/redismodule.c"
    };
    cc::Build::new()
        .file(stub)
        .include("include/")
        .compile("libredismodule.a");
    // The cc module emits `rustc-link-lib=static=redismodule` for us.
}
