//! SIGHUP re-reads the key set (DEC-283, addendum-1 2.2): the live cipher swaps to the new
//! ring, and refuses a set that drops a version still in use (keeping the old ring).

use app::ports::{SecretCipher, SecretSlot};
use auth::secrets::{KeyOptions, KeyRing, LiveKeys, ReloadError, read_keys};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use zeroize::Zeroizing;

const SLOT: SecretSlot<'static> = SecretSlot {
    purpose: "connector_credential",
    row: "018f0000-0000-7000-8000-000000000009",
    field: "secret",
};

fn line(v: u16, byte: u8) -> String {
    format!("v{v}:{}", STANDARD.encode([byte; 32]))
}

#[test]
fn a_reload_swaps_the_ring_and_old_values_stay_readable() {
    let live = LiveKeys::new(KeyRing::parse(&line(1, 7)).unwrap());
    let old = live.seal(SLOT, b"tmdb");
    assert_eq!((live.active_version(), live.versions()), (1, vec![1]));
    let next = KeyRing::parse(&format!("{}\n{}", line(2, 8), line(1, 7))).unwrap();
    live.replace(next, &[1]).unwrap();
    assert_eq!((live.active_version(), live.versions()), (2, vec![2, 1]));
    assert_eq!(live.open(SLOT, &old).unwrap(), b"tmdb");
    let new = live.seal(SLOT, b"igdb");
    assert_eq!(auth::secrets::blob_version(&new), Some(2));
}

#[test]
fn a_set_that_drops_a_version_in_use_is_refused_and_the_old_ring_kept() {
    let live = LiveKeys::new(KeyRing::parse(&format!("{}\n{}", line(2, 8), line(1, 7))).unwrap());
    let blob = live.seal(SLOT, b"x");
    let only_v3 = KeyRing::parse(&line(3, 9)).unwrap();
    assert_eq!(
        live.replace(only_v3, &[1, 2]).unwrap_err(),
        ReloadError::DropsVersionInUse(vec![1, 2])
    );
    assert_eq!(live.versions(), vec![2, 1], "old ring kept");
    assert_eq!(live.open(SLOT, &blob).unwrap(), b"x");
    // dropping v1 is fine once nothing uses it
    live.replace(KeyRing::parse(&line(2, 8)).unwrap(), &[2])
        .unwrap();
    assert_eq!(live.versions(), vec![2]);
}

#[test]
fn read_keys_reads_the_file_then_the_environment_and_never_generates() {
    let dir = std::env::temp_dir().join(format!("reload-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("secrets.key");
    let opts = KeyOptions {
        key_file: path.clone(),
        env_keys: Some(Zeroizing::new(line(4, 1))),
        ephemeral: false,
    };
    assert_eq!(read_keys(&opts).unwrap().versions(), vec![4]);
    std::fs::write(&path, format!("{}\n{}\n", line(6, 2), line(5, 3))).unwrap();
    assert_eq!(read_keys(&opts).unwrap().versions(), vec![6, 5]);
    let none = KeyOptions {
        key_file: dir.join("absent.key"),
        env_keys: None,
        ephemeral: false,
    };
    assert!(read_keys(&none).is_err());
    assert!(!dir.join("absent.key").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn rotate_adds_a_new_active_version_and_prune_drops_unused_ones() {
    let dir = std::env::temp_dir().join(format!("rotate-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("secrets.key");
    std::fs::write(&path, format!("{}\n", line(1, 7))).unwrap();
    let old = KeyRing::parse(&line(1, 7))
        .unwrap()
        .encrypt(SLOT, b"v1 value");
    assert_eq!(auth::secrets::rotate_key_file(&path).unwrap(), 2);
    let opts = KeyOptions {
        key_file: path.clone(),
        env_keys: None,
        ephemeral: false,
    };
    let ring = read_keys(&opts).unwrap();
    assert_eq!((ring.active_version(), ring.versions()), (2, vec![2, 1]));
    assert_eq!(&*ring.decrypt(SLOT, &old).unwrap(), b"v1 value");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    // v1 still in use: kept; then unused: dropped; the active version is never dropped
    assert_eq!(
        auth::secrets::prune_key_file(&path, &[1]).unwrap(),
        Vec::<u16>::new()
    );
    assert_eq!(auth::secrets::prune_key_file(&path, &[]).unwrap(), vec![1]);
    assert_eq!(read_keys(&opts).unwrap().versions(), vec![2]);
    std::fs::remove_dir_all(&dir).unwrap();
}
