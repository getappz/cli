use std::fs;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedPayload<T> {
    pub fetched_at: u64,
    pub data: T,
}

pub fn load_cached<T: Serialize + DeserializeOwned + Clone>(
    cache_path: &Path,
    ttl_seconds: u64,
    offline: bool,
    now: u64,
    fetch: impl FnOnce() -> Result<String, String>,
    parse: impl FnOnce(&str) -> Result<T, String>,
) -> Result<T, String> {
    let cache_exists = cache_path.exists();

    if !offline && (!cache_exists || stale(cache_path, ttl_seconds, now)) {
        match fetch() {
            Ok(raw) => match parse(&raw) {
                Ok(data) => {
                    let payload = CachedPayload {
                        fetched_at: now,
                        data: data.clone(),
                    };
                    if let Some(parent) = cache_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    if let Ok(json) = serde_json::to_string(&payload) {
                        let _ = fs::write(cache_path, &json);
                    }
                    return Ok(data);
                }
                Err(parse_err) => {
                    if let Ok(cached) = read_cache::<T>(cache_path) {
                        eprintln!("warning: registry parse failed, using stale cache: {parse_err}");
                        return Ok(cached.data);
                    }
                    return Err(parse_err);
                }
            },
            Err(fetch_err) => {
                if let Ok(cached) = read_cache::<T>(cache_path) {
                    eprintln!("warning: registry fetch failed, using stale cache: {fetch_err}");
                    return Ok(cached.data);
                }
                return Err(fetch_err);
            }
        }
    }

    if cache_exists {
        read_cache::<T>(cache_path).map(|p| p.data)
    } else {
        Err("no cache available and --offline is set (or TTL not expired)".to_string())
    }
}

fn read_cache<T: DeserializeOwned>(cache_path: &Path) -> Result<CachedPayload<T>, String> {
    let raw = fs::read_to_string(cache_path).map_err(|e| format!("cache read error: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("cache parse error: {e}"))
}

fn stale(cache_path: &Path, ttl_seconds: u64, now: u64) -> bool {
    read_cache::<serde_json::Value>(cache_path)
        .ok()
        .and_then(|p| p.fetched_at.checked_add(ttl_seconds))
        .map(|expires| now > expires)
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_dir() -> std::path::PathBuf {
        std::env::temp_dir().join("appz-core-test-registry-cache")
    }

    fn setup() {
        // Tests run in parallel and share this directory — do not
        // remove_dir_all it here, that races with sibling tests writing
        // their own (uniquely named) cache files. create_dir_all is
        // idempotent and safe to call concurrently.
        fs::create_dir_all(cache_dir()).unwrap();
    }

    fn cache_path(name: &str) -> std::path::PathBuf {
        let p = cache_dir().join(name);
        let _ = fs::remove_file(&p);
        p
    }

    fn ok_fetch() -> impl FnOnce() -> Result<String, String> {
        || Ok(r#"{"version":1,"frameworks":[]}"#.to_string())
    }

    fn ok_parse<T>() -> impl FnOnce(&str) -> Result<T, String>
    where
        T: DeserializeOwned,
    {
        |raw| serde_json::from_str(raw).map_err(|e| e.to_string())
    }

    fn fail_fetch() -> impl FnOnce() -> Result<String, String> {
        || Err("network error".to_string())
    }

    fn fail_parse() -> impl FnOnce() -> Result<String, String> {
        || Ok("not-json".to_string())
    }

    #[test]
    fn test_cache_miss_fetches_and_caches() {
        setup();
        let path = cache_path("miss.json");
        let now = 1000u64;

        let result = load_cached(
            &path,
            3600,
            false,
            now,
            ok_fetch(),
            ok_parse::<serde_json::Value>(),
        );
        assert!(result.is_ok());
        assert!(path.exists(), "cache file should be created");

        let cached: CachedPayload<serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(cached.fetched_at, now);
    }

    #[test]
    fn test_cache_hit_within_ttl() {
        setup();
        let path = cache_path("hit.json");
        let now = 1000u64;

        let payload = CachedPayload {
            fetched_at: 900,
            data: serde_json::json!({"version": 1}),
        };
        fs::write(&path, serde_json::to_string(&payload).unwrap()).unwrap();

        let result = load_cached(
            &path,
            3600,
            false,
            now,
            fail_fetch(),
            ok_parse::<serde_json::Value>(),
        );
        assert!(result.is_ok(), "should use cache within TTL");
    }

    #[test]
    fn test_ttl_expired_refetches() {
        setup();
        let path = cache_path("expired.json");
        let now = 2000u64;

        let payload = CachedPayload {
            fetched_at: 1000,
            data: serde_json::json!({"version": "old"}),
        };
        fs::write(&path, serde_json::to_string(&payload).unwrap()).unwrap();

        let result = load_cached(
            &path,
            500,
            false,
            now,
            ok_fetch(),
            ok_parse::<serde_json::Value>(),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["version"], 1, "should refetch when TTL expired");
    }

    #[test]
    fn test_offline_uses_cache() {
        setup();
        let path = cache_path("offline.json");
        let now = 2000u64;

        let payload = CachedPayload {
            fetched_at: 1000,
            data: serde_json::json!({"version": "cached"}),
        };
        fs::write(&path, serde_json::to_string(&payload).unwrap()).unwrap();

        let result = load_cached(
            &path,
            500,
            true,
            now,
            fail_fetch(),
            ok_parse::<serde_json::Value>(),
        );
        assert!(result.is_ok(), "offline should use cache");
        assert_eq!(result.unwrap()["version"], "cached");
    }

    #[test]
    fn test_offline_no_cache_errors() {
        setup();
        let path = cache_path("offline-nocache.json");

        let result =
            load_cached::<serde_json::Value>(&path, 3600, true, 1000, fail_fetch(), |_| {
                Err("nope".to_string())
            });
        assert!(result.is_err(), "offline with no cache should error");
    }

    #[test]
    fn test_fetch_error_falls_back_to_stale_cache() {
        setup();
        let path = cache_path("stale-fallback.json");
        let now = 2000u64;

        let payload = CachedPayload {
            fetched_at: 1000,
            data: serde_json::json!({"version": "stale"}),
        };
        fs::write(&path, serde_json::to_string(&payload).unwrap()).unwrap();

        let result = load_cached(
            &path,
            500,
            false,
            now,
            fail_fetch(),
            ok_parse::<serde_json::Value>(),
        );
        assert!(
            result.is_ok(),
            "fetch error should fall back to stale cache"
        );
    }

    #[test]
    fn test_parse_error_falls_back_to_stale_cache() {
        setup();
        let path = cache_path("parse-stale.json");
        let now = 2000u64;

        let payload = CachedPayload {
            fetched_at: 1000,
            data: serde_json::json!({"version": "stale"}),
        };
        fs::write(&path, serde_json::to_string(&payload).unwrap()).unwrap();

        let result = load_cached::<serde_json::Value>(&path, 500, false, now, fail_parse(), |_| {
            Err("bad data".to_string())
        });
        assert!(
            result.is_ok(),
            "parse error should fall back to stale cache"
        );
    }

    #[test]
    fn test_no_cache_no_network_errors() {
        setup();
        let path = cache_path("no-cache-no-net.json");

        let result =
            load_cached::<serde_json::Value>(&path, 3600, false, 1000, fail_fetch(), |_| {
                Err("nope".to_string())
            });
        assert!(result.is_err(), "no cache + fetch fail should error");
    }
}
