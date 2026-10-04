use std::fs;

use serde_json::Value;
use tempfile::TempDir;

use crate::*;

struct TestStore {
    _tempdir: TempDir,
    auth_path: PathBuf,
    store: SecretStore,
}

impl TestStore {
    fn new() -> Self {
        let tempdir = TempDir::new().expect("tempdir");
        let auth_path = tempdir.path().join(".notslack").join("auth.json");
        let store = SecretStore::for_test(auth_path.clone());
        Self {
            _tempdir: tempdir,
            auth_path,
            store,
        }
    }

    fn json(&self) -> Value {
        let contents = fs::read_to_string(&self.auth_path).expect("auth file contents");
        serde_json::from_str(&contents).expect("auth file json")
    }
}

#[gpui::test]
fn read_returns_none_when_credentials_file_is_missing() {
    let fixture = TestStore::new();

    assert!(!fixture.auth_path.exists());
    assert!(!fixture
        .store
        .has_refresh_token("acme|notslack-native")
        .expect("probe refresh token"));
    assert_eq!(
        fixture
            .store
            .read_refresh_token("acme|notslack-native", "access notslack workspace")
            .expect("read refresh token"),
        None
    );
}

#[gpui::test]
fn upsert_and_read_round_trip_refresh_tokens() {
    let fixture = TestStore::new();
    fixture
        .store
        .upsert_refresh_token("acme|notslack-native", "refresh-token-1")
        .expect("store refresh token");

    assert!(fixture.auth_path.exists());
    assert!(fixture
        .store
        .has_refresh_token("acme|notslack-native")
        .expect("probe stored refresh token"));
    assert_eq!(
        fixture
            .store
            .read_refresh_token("acme|notslack-native", "access notslack workspace")
            .expect("read refresh token")
            .as_deref(),
        Some("refresh-token-1")
    );
    assert_eq!(
        fixture.json()["refresh_tokens"]["acme|notslack-native"].as_str(),
        Some("refresh-token-1")
    );

    fixture
        .store
        .upsert_refresh_token("acme|notslack-native", "refresh-token-2")
        .expect("overwrite refresh token");
    assert_eq!(
        fixture
            .store
            .read_refresh_token("acme|notslack-native", "access notslack workspace")
            .expect("read overwritten refresh token")
            .as_deref(),
        Some("refresh-token-2")
    );
}

#[gpui::test]
fn upsert_read_and_remove_round_trip_access_tokens() {
    let fixture = TestStore::new();
    fixture
        .store
        .upsert_access_token("acme|notslack-native", "access-token-1", 1_800_000_000)
        .expect("store access token");

    assert_eq!(
        fixture
            .store
            .read_access_token("acme|notslack-native")
            .expect("read access token"),
        Some(StoredAccessToken {
            access_token: "access-token-1".to_string(),
            expires_at_epoch_seconds: 1_800_000_000,
        })
    );
    assert_eq!(
        fixture.json()["access_tokens"]["acme|notslack-native"]["access_token"].as_str(),
        Some("access-token-1")
    );

    fixture
        .store
        .upsert_access_token("acme|notslack-native", "access-token-2", 1_800_000_300)
        .expect("overwrite access token");
    assert_eq!(
        fixture
            .store
            .read_access_token("acme|notslack-native")
            .expect("read overwritten access token"),
        Some(StoredAccessToken {
            access_token: "access-token-2".to_string(),
            expires_at_epoch_seconds: 1_800_000_300,
        })
    );

    fixture
        .store
        .remove_access_token("acme|notslack-native")
        .expect("remove access token");
    assert_eq!(
        fixture
            .store
            .read_access_token("acme|notslack-native")
            .expect("read removed access token"),
        None
    );
}

#[gpui::test]
fn upsert_and_read_round_trip_generic_secrets() {
    let fixture = TestStore::new();
    fixture
        .store
        .upsert_secret("slack|user-token", "top-secret")
        .expect("store secret");

    assert_eq!(
        fixture
            .store
            .read_secret("slack|user-token")
            .expect("read secret")
            .as_deref(),
        Some("top-secret")
    );
    assert_eq!(
        fixture.json()["secrets"]["slack|user-token"].as_str(),
        Some("top-secret")
    );
}

#[gpui::test]
fn refresh_tokens_are_isolated() {
    let fixture = TestStore::new();
    fixture
        .store
        .upsert_refresh_token("acme-a|notslack-native", "refresh-token-a")
        .expect("store token a");
    fixture
        .store
        .upsert_refresh_token("acme-b|notslack-native", "refresh-token-b")
        .expect("store token b");

    assert_eq!(
        fixture
            .store
            .read_refresh_token("acme-a|notslack-native", "access notslack workspace")
            .expect("read token a")
            .as_deref(),
        Some("refresh-token-a")
    );
    assert_eq!(
        fixture
            .store
            .read_refresh_token("acme-b|notslack-native", "access notslack workspace")
            .expect("read token b")
            .as_deref(),
        Some("refresh-token-b")
    );
}

#[gpui::test]
fn read_refresh_token_accepts_any_access_reason_text() {
    let fixture = TestStore::new();
    fixture
        .store
        .upsert_refresh_token("acme|notslack-native", "refresh-token-1")
        .expect("store refresh token");
    assert_eq!(
        fixture
            .store
            .read_refresh_token("acme|notslack-native", "device authentication canceled")
            .expect("read refresh token with arbitrary access reason")
            .as_deref(),
        Some("refresh-token-1")
    );
}

#[gpui::test]
fn refresh_tokens_and_cache_keys_use_separate_storage() {
    let fixture = TestStore::new();
    fixture
        .store
        .upsert_refresh_token("acme|notslack-native", "refresh-token-1")
        .expect("store refresh token");
    fixture
        .store
        .upsert_cache_key("acme|notslack-native", "cache-key-1")
        .expect("store cache key");

    assert_eq!(
        fixture
            .store
            .read_refresh_token("acme|notslack-native", "access notslack workspace")
            .expect("read refresh token")
            .as_deref(),
        Some("refresh-token-1")
    );
    assert_eq!(
        fixture
            .store
            .read_cache_key("acme|notslack-native")
            .expect("read cache key")
            .as_deref(),
        Some("cache-key-1")
    );
    let json = fixture.json();
    assert_eq!(
        json["refresh_tokens"]["acme|notslack-native"].as_str(),
        Some("refresh-token-1")
    );
    assert_eq!(
        json["cache_keys"]["acme|notslack-native"].as_str(),
        Some("cache-key-1")
    );
}

#[gpui::test]
fn cache_keys_are_isolated() {
    let fixture = TestStore::new();
    fixture
        .store
        .upsert_cache_key("acme-a|notslack-native|mail", "cache-key-a")
        .expect("store cache key a");
    fixture
        .store
        .upsert_cache_key("acme-b|notslack-native|mail", "cache-key-b")
        .expect("store cache key b");

    assert_eq!(
        fixture
            .store
            .read_cache_key("acme-a|notslack-native|mail")
            .expect("read cache key a")
            .as_deref(),
        Some("cache-key-a")
    );
    assert_eq!(
        fixture
            .store
            .read_cache_key("acme-b|notslack-native|mail")
            .expect("read cache key b")
            .as_deref(),
        Some("cache-key-b")
    );
}

#[gpui::test]
fn corrupt_credentials_file_fails_loudly() {
    let fixture = TestStore::new();
    fs::create_dir_all(fixture.auth_path.parent().expect("auth path parent")).expect("auth dir");
    fs::write(&fixture.auth_path, "not json").expect("corrupt auth file");

    let error = fixture
        .store
        .read_secret("slack|user-token")
        .expect_err("corrupt credentials should fail");

    assert!(
        format!("{error:#}").contains("failed to decode notslack credentials"),
        "unexpected error: {error:#}"
    );
}

#[gpui::test]
fn unsupported_credentials_version_fails_loudly() {
    let fixture = TestStore::new();
    fs::create_dir_all(fixture.auth_path.parent().expect("auth path parent")).expect("auth dir");
    fs::write(
        &fixture.auth_path,
        r#"{"version":2,"refresh_tokens":{},"secrets":{},"cache_keys":{}}"#,
    )
    .expect("unsupported version auth file");

    let error = fixture
        .store
        .read_secret("slack|user-token")
        .expect_err("unsupported credentials version should fail");

    assert!(
        format!("{error:#}").contains("unsupported notslack credentials version 2"),
        "unexpected error: {error:#}"
    );
}

#[cfg(unix)]
#[gpui::test]
fn credentials_file_uses_private_unix_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = TestStore::new();
    fixture
        .store
        .upsert_secret("slack|user-token", "top-secret")
        .expect("store secret");

    let auth_dir = fixture.auth_path.parent().expect("auth path parent");
    let dir_mode = fs::metadata(auth_dir)
        .expect("auth dir metadata")
        .permissions()
        .mode()
        & 0o777;
    let file_mode = fs::metadata(&fixture.auth_path)
        .expect("auth file metadata")
        .permissions()
        .mode()
        & 0o777;

    assert_eq!(dir_mode, 0o700);
    assert_eq!(file_mode, 0o600);
}
