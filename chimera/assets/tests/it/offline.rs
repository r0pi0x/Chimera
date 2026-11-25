//! A lookup never hangs on an asset service it can't reach (offline, a
//! sandbox without network): the install lookup's fallback to the service
//! (`AssetSettings::game_path`, as `LocalConfig::game_path_in` asks it)
//! fails within `remote::CONNECT_TIMEOUT` and a little, says the service
//! is unreachable, and the other games' lookups then fail at once
//! (`remote::unreachable`) instead of waiting each.

use std::{io::ErrorKind, net::TcpListener, time::Instant};

use chimera_assets::{
    AssetSettings, Remote, RemoteConfig, Store,
    remote::{CONNECT_TIMEOUT, UNREACHABLE_FOR},
};

use super::scratch;

fn config(url: &str) -> RemoteConfig {
    RemoteConfig { url: url.into(), token: "a token".into(), key: None, fingerprint: None }
}

/// Generous slack over the connect timeout for a loaded machine.
fn bound() -> std::time::Duration {
    CONNECT_TIMEOUT * 2 + std::time::Duration::from_secs(2)
}

#[test]
fn the_install_lookup_s_fallback_fails_fast_on_a_service_that_never_answers() {
    // A port that takes connections (the kernel's backlog) but never
    // answers: the TLS handshake stalls, as behind a sandbox's or a
    // captive portal's proxy.
    let silent = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("https://127.0.0.1:{}", silent.local_addr().unwrap().port());
    let dir = scratch("offline_lookup");
    let settings = AssetSettings {
        store: Some(Store::new(dir.join("store"))),
        use_install: true,
        service_first: true,
        builds: Default::default(),
        remote: Some(config(&url)),
        lock: None,
    };
    let start = Instant::now();
    assert_eq!(settings.game_path("first", || None), None, "no install and no service: none");
    let first = start.elapsed();
    assert!(first < bound(), "the first lookup took {first:?} (connect timeout {CONNECT_TIMEOUT:?})");
    let start = Instant::now();
    for game in ["second", "third", "fourth"] {
        assert_eq!(settings.game_path(game, || None), None);
    }
    let rest = start.elapsed();
    assert!(rest.as_millis() < 500, "the other games waited {rest:?}: the service is known unreachable");
    // An install found meanwhile is still used.
    let found = dir.join("install");
    assert_eq!(settings.game_path("fifth", || Some(found.clone())), Some(found));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unreachable_address_fails_fast_with_a_clear_message() {
    // TEST-NET-1 (never routed: dropped or no route) and a name that never
    // resolves (`.invalid`).
    for url in ["https://192.0.2.1:9", "https://chimera-asset-service.invalid"] {
        let remote = Remote::new(config(url)).unwrap();
        let start = Instant::now();
        let e = remote.latest_build("game").unwrap_err();
        let took = start.elapsed();
        assert!(took < bound(), "{url}: {took:?}");
        assert_eq!(e.kind(), ErrorKind::NotConnected, "{url}: {e}");
        assert!(e.to_string().contains("unreachable"), "{url}: {e}");
        // Asked again within UNREACHABLE_FOR: at once, saying why.
        let start = Instant::now();
        let again = remote.latest_build("game").unwrap_err();
        assert!(start.elapsed().as_millis() < 100, "{url}: asked again");
        assert!(again.to_string().contains(&format!("not asked again for {} s", UNREACHABLE_FOR.as_secs())), "{again}");
    }
}
