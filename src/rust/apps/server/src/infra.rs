//! Production adapters for the small infrastructure ports.

use std::sync::Mutex;

use app::ports::{Clock, IdGen};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use domain::hlc::Hlc;

/// Wall clock plus this node's hybrid logical clock (D-12).
pub struct SystemClock {
    hlc: Mutex<Hlc>,
}

impl SystemClock {
    /// `node` distinguishes writers with equal timestamps (random per process).
    #[must_use]
    pub fn new(node: u32) -> Self {
        Self {
            hlc: Mutex::new(Hlc::new(0, 0, node)),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        let mut b = [0u8; 4];
        getrandom::fill(&mut b).expect("os randomness");
        Self::new(u32::from_le_bytes(b))
    }
}

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn hlc(&self) -> Hlc {
        let now_ms = u64::try_from(Utc::now().timestamp_millis()).unwrap_or(0);
        let mut h = self.hlc.lock().expect("hlc");
        *h = h.tick(now_ms);
        *h
    }

    fn observe(&self, remote: Hlc) {
        let now_ms = u64::try_from(Utc::now().timestamp_millis()).unwrap_or(0);
        let mut h = self.hlc.lock().expect("hlc");
        *h = h.receive(remote, now_ms);
    }
}

/// `UUIDv7` ids, 256-bit random tokens, blake3 token hashes.
#[derive(Default)]
pub struct SystemIds;

impl IdGen for SystemIds {
    fn new_id(&self) -> uuid::Uuid {
        uuid::Uuid::now_v7()
    }

    fn new_token(&self) -> String {
        let mut b = [0u8; 32];
        getrandom::fill(&mut b).expect("os randomness");
        URL_SAFE_NO_PAD.encode(b)
    }

    fn hash_token(&self, token: &str) -> String {
        blake3::hash(token.as_bytes()).to_hex().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hlc_is_monotonic() {
        let c = SystemClock::new(7);
        let a = c.hlc();
        let b = c.hlc();
        assert!(b > a);
    }

    #[test]
    fn tokens_are_random_and_hashed() {
        let ids = SystemIds;
        let (a, b) = (ids.new_token(), ids.new_token());
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert_eq!(ids.hash_token(&a), ids.hash_token(&a));
        assert_ne!(ids.hash_token(&a), a);
    }
}

/// What an operator should hear at start-up about how the instance is reachable (SEC-6,
/// OPS-2); empty when nothing is odd. `public_url` and `bind` are the `server` settings,
/// `secure_cookies` the effective cookie setting.
///
/// - Plain HTTP at the public URL while the server listens beyond loopback: sign-in data
///   travels unencrypted unless a TLS proxy is in front.
/// - Secure cookies with a plain-HTTP public URL that is not this machine: browsers drop
///   the session cookie, nobody can sign in.
#[must_use]
pub fn exposure_warnings(public_url: &str, bind: &str, secure_cookies: bool) -> Vec<&'static str> {
    let Ok(url) = url::Url::parse(public_url) else {
        return Vec::new();
    };
    if url.scheme() != "http" {
        return Vec::new();
    }
    let local = |host: &str| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    };
    let mut out = Vec::new();
    let listens_beyond = bind
        .parse::<std::net::SocketAddr>()
        .is_ok_and(|a| !a.ip().is_loopback());
    if listens_beyond {
        out.push(
            "server.public_url is plain HTTP and the server listens beyond loopback: put a \
             reverse proxy with TLS in front and set APP__SERVER__PUBLIC_URL to its https address",
        );
    }
    if secure_cookies && !url.host_str().is_some_and(local) {
        out.push(
            "auth.secure_cookies is on but server.public_url is plain HTTP: browsers will not \
             send the session cookie and nobody can sign in; use https, or set \
             APP__AUTH__SECURE_COOKIES=false for a test on a trusted network",
        );
    }
    out
}

#[cfg(test)]
mod exposure_tests {
    use super::exposure_warnings;

    #[test]
    fn https_or_loopback_is_quiet() {
        assert_eq!(
            exposure_warnings("https://tracker.example.org", "0.0.0.0:3000", true).len(),
            0
        );
        assert_eq!(
            exposure_warnings("http://localhost:3000", "127.0.0.1:3000", true).len(),
            0
        );
        assert_eq!(
            exposure_warnings("http://localhost:3000", "[::1]:3000", false).len(),
            0
        );
    }

    #[test]
    fn plain_http_beyond_loopback_is_named() {
        // the compose default before PUBLIC_URL is set: only the first warning (the cookie
        // works on localhost)
        let w = exposure_warnings("http://localhost:3000", "0.0.0.0:3000", true);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("plain HTTP"), "{w:?}");
        // a LAN test over http with secure cookies: nobody can sign in
        let w = exposure_warnings("http://192.168.1.20:3000", "0.0.0.0:3000", true);
        assert_eq!(w.len(), 2, "{w:?}");
        assert!(w[1].contains("secure_cookies"), "{w:?}");
        let w = exposure_warnings("http://192.168.1.20:3000", "0.0.0.0:3000", false);
        assert_eq!(w.len(), 1, "{w:?}");
        // an unparsable setting is not this function's business
        assert_eq!(exposure_warnings("nonsense", "nonsense", true).len(), 0);
    }
}

/// Where the Leptos components look for the hash file of the bundle (`hash.txt`, written by
/// cargo-leptos with `hash-files`): next to this executable, unless `name` is a full path.
#[must_use]
pub fn hash_file_path(name: &str) -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_default()
        .join(name)
}

/// What the hash file says the bundle files are called: `<output name>.<hash>.<ext>` for
/// each `js`, `wasm` and `css` line (the lines the pages read; cargo-leptos writes all three).
fn hashed_names(hashes: &str, output_name: &str) -> Vec<String> {
    hashes
        .lines()
        .filter_map(|line| line.trim().split_once(':'))
        .filter(|(kind, _)| matches!(*kind, "js" | "wasm" | "css"))
        .map(|(kind, hash)| format!("{output_name}.{}.{kind}", hash.trim()))
        .collect()
}

/// Do the pages name the bundle files with their content hashes (OPS-8)? They do whenever
/// the hash file is there and the files it names are in `pkg` (`<site root>/pkg`), whatever
/// `LEPTOS_HASH_FILES` says (`configured`): a site built with hashes has no file under the
/// plain names, and a page that asks for `app.js` would stay without scripts.
///
/// The hash file is not trusted on its own (review SEC1-6, DEC-663): it lies next to the
/// executable and the site root is another directory, so one can be older than the other (a
/// release build wipes the site and writes its own hash file; a build with `hash-files`
/// switched off leaves the old hash file behind). When a named file is missing and the site
/// has the plain `<output name>.js`, the pages use the plain names and a warning says so.
///
/// # Errors
/// The server does not start when its pages would name files that do not exist: hashed
/// names are asked for and the hash file is missing, or the hash file names files the site
/// does not have and the plain names are not there either.
pub fn bundle_hashes(
    configured: bool,
    hash_file: &std::path::Path,
    pkg: &std::path::Path,
    output_name: &str,
) -> anyhow::Result<bool> {
    let Ok(hashes) = std::fs::read_to_string(hash_file) else {
        if configured {
            anyhow::bail!(
                "LEPTOS_HASH_FILES is on, but the hash file {} is missing: copy `hash.txt` of \
                 the build next to the server executable (or name it with \
                 LEPTOS_HASH_FILE_NAME)",
                hash_file.display()
            );
        }
        return Ok(false);
    };
    let names = hashed_names(&hashes, output_name);
    let missing: Vec<&str> = names
        .iter()
        .filter(|name| !pkg.join(name).is_file())
        .map(String::as_str)
        .collect();
    let scripts = ["js", "wasm"]
        .iter()
        .all(|ext| names.iter().any(|n| n.ends_with(&format!(".{ext}"))));
    if scripts && missing.is_empty() {
        return Ok(true);
    }
    let what = if scripts {
        format!("names files that are not in {}", pkg.display())
    } else {
        "has no `js` and `wasm` line".to_string()
    };
    if pkg.join(format!("{output_name}.js")).is_file() {
        tracing::warn!(
            hash_file = %hash_file.display(),
            missing = ?missing,
            "the hash file {what}; the site has `{output_name}.js`, so the pages use the \
             names without hashes (delete the stale hash file to silence this)"
        );
        return Ok(false);
    }
    anyhow::bail!(
        "the hash file {} {what} (missing: {}): the site root and the hash file come from \
         different builds. Build both again for the profile of this executable (`cargo \
         leptos build`, with `--release` for a release executable), or copy the `hash.txt` \
         that belongs to the site next to the server executable",
        hash_file.display(),
        if missing.is_empty() {
            "nothing".to_string()
        } else {
            missing.join(", ")
        }
    );
}

#[cfg(test)]
mod bundle_tests {
    use super::*;

    fn site() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("hashes-{}", uuid::Uuid::now_v7()));
        let pkg = dir.join("site/pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        (dir.clone(), dir.join("hash.txt"), pkg)
    }

    #[test]
    fn the_hash_file_decides_and_a_missing_one_is_an_error_when_asked_for() {
        let (dir, file, pkg) = site();
        assert!(
            !bundle_hashes(false, &file, &pkg, "app").unwrap(),
            "a build without hashes"
        );
        let e = bundle_hashes(true, &file, &pkg, "app")
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("hash.txt") && e.contains("LEPTOS_HASH_FILES"),
            "{e}"
        );
        std::fs::write(&file, "js: a\nwasm: b\ncss: c\n").unwrap();
        for name in ["app.a.js", "app.b.wasm", "app.c.css"] {
            std::fs::write(pkg.join(name), "x").unwrap();
        }
        assert!(bundle_hashes(true, &file, &pkg, "app").unwrap());
        assert!(
            bundle_hashes(false, &file, &pkg, "app").unwrap(),
            "the file is enough"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Review SEC1-6: a hash file from another build than the site root.
    #[test]
    fn a_hash_file_that_names_missing_files_is_not_followed() {
        let (dir, file, pkg) = site();
        std::fs::write(&file, "js: old1\nwasm: old2\ncss: old3\n").unwrap();
        // the site of a newer build: other hashes, no plain names
        for name in ["app.new1.js", "app.new2.wasm", "app.new3.css"] {
            std::fs::write(pkg.join(name), "x").unwrap();
        }
        for configured in [true, false] {
            let e = bundle_hashes(configured, &file, &pkg, "app")
                .unwrap_err()
                .to_string();
            for part in ["hash.txt", "app.old1.js", "app.old2.wasm", "app.old3.css"] {
                assert!(e.contains(part), "{part}: {e}");
            }
            assert!(e.contains("different builds"), "{e}");
        }
        // one file of three is enough to refuse
        std::fs::write(pkg.join("app.old1.js"), "x").unwrap();
        std::fs::write(pkg.join("app.old2.wasm"), "x").unwrap();
        let e = bundle_hashes(true, &file, &pkg, "app")
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("app.old3.css") && !e.contains("app.old1.js"),
            "{e}"
        );
        // a site built without hashes next to a stale hash file: the plain names are used
        std::fs::write(pkg.join("app.js"), "x").unwrap();
        for configured in [true, false] {
            assert!(!bundle_hashes(configured, &file, &pkg, "app").unwrap());
        }
        // a hash file without the script lines names nothing a page could load
        std::fs::remove_file(pkg.join("app.js")).unwrap();
        std::fs::write(&file, "css: old3\n").unwrap();
        std::fs::write(pkg.join("app.old3.css"), "x").unwrap();
        let e = bundle_hashes(true, &file, &pkg, "app")
            .unwrap_err()
            .to_string();
        assert!(e.contains("`js` and `wasm`"), "{e}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_hash_file_is_looked_for_next_to_the_executable() {
        let p = hash_file_path("hash.txt");
        assert_eq!(p.parent(), std::env::current_exe().unwrap().parent());
        assert_eq!(
            hash_file_path("/srv/site/hash.txt"),
            std::path::Path::new("/srv/site/hash.txt")
        );
    }
}
