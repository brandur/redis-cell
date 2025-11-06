#![cfg(feature = "e2e-test")]

use std::time::Duration;

use redis::{Cmd, Value};

#[tokio::test]
async fn it_works() {
    let (_container, mut client) = utils::setup().await;
    let mut cmd = Cmd::new();
    cmd.arg("CL.THROTTLE")
        .arg("user123") // for this key
        .arg(0) // with max burst
        .arg(1) // regenerate that many tokens
        .arg(2) // within that many seconds
        .arg(1); // and only apply 1 token per request (default)

    // we are first allowed, but ...
    let res = client
        .send_packed_command(&cmd)
        .await
        .unwrap()
        .into_sequence()
        .unwrap();
    let (throttled, total, remaining, restry_after_sesc, reset_after_secs) =
        (&res[0], &res[1], &res[2], &res[3], &res[4]);
    assert_eq!(*throttled, Value::Int(0)); // i.e. allowed
    assert_eq!(*total, Value::Int(1)); // burst + 1
    assert_eq!(*remaining, Value::Int(0)); // total - applied (1 per request)
    assert_eq!(*restry_after_sesc, Value::Int(-1)); // always -1 for allowed
    assert_eq!(*reset_after_secs, Value::Int(2));

    // ... throttled immediately after this, since we've run out of budget
    let res = client
        .send_packed_command(&cmd)
        .await
        .unwrap()
        .into_sequence()
        .unwrap();
    let (throttled, total, remaining, retry_after, reset_after) =
        (&res[0], &res[1], &res[2], &res[3], &res[4]);
    assert_eq!(*throttled, Value::Int(1)); // i.e. blocked
    assert_eq!(*total, Value::Int(1)); // burst + 1
    assert_eq!(*remaining, Value::Int(0)); // total - applied (1 per request) * 2 requests
    assert_eq!(*retry_after, Value::Int(2)); // 1 token every 2 seconds
    assert_eq!(*reset_after, Value::Int(2));

    // let's await, and ...
    let Value::Int(retry_after_secs) = *retry_after else {
        unreachable!("As per Redis Cell API and our assetion above");
    };

    // ... retry our request
    tokio::time::sleep(Duration::from_secs(retry_after_secs as u64)).await;
    let res = client
        .send_packed_command(&cmd)
        .await
        .unwrap()
        .into_sequence()
        .unwrap();
    let (throttled, total, remaining, retry_after, reset_after) =
        (&res[0], &res[1], &res[2], &res[3], &res[4]);
    assert_eq!(*throttled, Value::Int(0)); // NB
    assert_eq!(*total, Value::Int(1)); // burst + 1
    assert_eq!(*remaining, Value::Int(0)); // total - applied (1 per request) * 2 requests
    assert_eq!(*retry_after, Value::Int(-1)); // again, it's alwaus -1 for allowed
    assert_eq!(*reset_after, Value::Int(2));
}

mod utils {
    use redis::aio::ConnectionManager;
    use std::sync::LazyLock;
    use testcontainers::ContainerAsync;
    use testcontainers::core::IntoContainerPort as _;
    use testcontainers::runners::AsyncRunner;
    use testcontainers::{GenericImage, core::WaitFor};

    static REDIS_CLIENT_MAX_RETRY: LazyLock<usize> = LazyLock::new(|| {
        std::env::var("REDIS_CLIENT_MAX_RETRY")
            .ok()
            .map(|val| {
                val.parse()
                    .expect("valid usize value to have been passed from env")
            })
            .unwrap_or(3)
    });
    static REDIS_CLIENT_MAX_DELAY: LazyLock<u64> = LazyLock::new(|| {
        std::env::var("REDIS_CLIENT_MAX_DELAY")
            .ok()
            .map(|val| {
                val.parse()
                    .expect("valid u64 value to have been passed from env")
            })
            .unwrap_or(1000)
    });

    pub(super) async fn setup() -> (ContainerAsync<GenericImage>, ConnectionManager) {
        let image_name = if cfg!(feature = "valkey") {
            "valkey-cell"
        } else {
            "redis-cell"
        };
        let container = GenericImage::new(image_name, "latest")
            .with_exposed_port(6379.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
            .start()
            .await
            .unwrap();
        let port = container.get_host_port_ipv4(6379).await.unwrap();
        let client = redis::Client::open(("localhost", port)).unwrap();
        let config = redis::aio::ConnectionManagerConfig::new()
            // empirically discovered: when using a custom runtime, e.g. Colima,
            // the connection will oftentimes not get established with the first
            // attempt even though the container is up and running and the `Ready
            // to accept connections` message has appeared in the container's
            // stdout; so we allow for a few retries but cap the delay time in
            // between those attempt so that the test does not hang; we also
            // make it possible to adjust those values via the environment
            .set_number_of_retries(*REDIS_CLIENT_MAX_RETRY)
            .set_max_delay(*REDIS_CLIENT_MAX_DELAY);
        let manager = redis::aio::ConnectionManager::new_with_config(client, config)
            .await
            .unwrap();
        (container, manager)
    }
}
