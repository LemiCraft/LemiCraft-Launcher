// One client for the whole launcher: open connections are reused, so the TLS handshake is paid once per host
use std::sync::OnceLock;
use std::time::Duration;

fn builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(30))
}

/// For worker threads (spawn_blocking); 20 s per request unless a call sets its own `.timeout()`
pub fn blocking() -> reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            // built on a thread of its own: a blocking client started inside an async task panics
            std::thread::spawn(|| {
                reqwest::blocking::Client::builder()
                    .connect_timeout(Duration::from_secs(10))
                    .timeout(Duration::from_secs(20))
                    .pool_idle_timeout(Duration::from_secs(90))
                    .tcp_keepalive(Duration::from_secs(30))
                    .build()
                    .unwrap_or_default()
            })
            .join()
            .unwrap_or_default()
        })
        .clone()
}

/// For async code; the caller sets the time of a request with `.timeout()`
pub fn asynchronous() -> reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| builder().build().unwrap_or_default()).clone()
}
