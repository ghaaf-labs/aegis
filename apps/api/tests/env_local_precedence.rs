//! F-ENV-2 — dotenvy load-order regression test.
//!
//! Pins the contract that production code in `apps/api/src/main.rs` and
//! `apps/api/src/bin/regime_backtest.rs` relies on:
//!
//! 1. `.env.local` (gitignored, personal overrides) is loaded first → wins.
//! 2. `.env` (committed defaults) is loaded second → fills remaining keys.
//! 3. Real env vars already set in the shell beat both, because dotenvy's
//!    non-overriding semantics never replace an existing OS env entry.
//!
//! Each test uses a unique env-var name and a per-test tmpdir so they can run
//! in parallel without polluting each other. We don't need an external
//! `tempfile` crate — `std::env::temp_dir() + (pid, nanos)` is sufficient
//! and keeps the dev-deps slim.

use std::path::{Path, PathBuf};

fn make_tmpdir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!(
        "aegis-env-test-{tag}-{}-{}",
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&dir).expect("create tmpdir");
    dir
}

fn write_env(dir: &Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).expect("write env file");
}

#[test]
fn env_local_wins_over_env_for_same_key() {
    let key = "F_ENV_2_TEST_OVERRIDE_A";
    std::env::remove_var(key);

    let dir = make_tmpdir("override");
    write_env(&dir, ".env.local", &format!("{key}=from_local\n"));
    write_env(&dir, ".env", &format!("{key}=from_env\n"));

    // Production load order: .env.local first (wins), .env second (fills).
    dotenvy::from_path(dir.join(".env.local")).unwrap();
    dotenvy::from_path(dir.join(".env")).unwrap();

    assert_eq!(std::env::var(key).unwrap(), "from_local");

    std::env::remove_var(key);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn env_fills_when_env_local_missing_key() {
    let key = "F_ENV_2_TEST_FILL_B";
    std::env::remove_var(key);

    let dir = make_tmpdir("fill");
    write_env(&dir, ".env.local", "OTHER_VAR=ignored\n");
    write_env(&dir, ".env", &format!("{key}=defaulted\n"));

    dotenvy::from_path(dir.join(".env.local")).unwrap();
    dotenvy::from_path(dir.join(".env")).unwrap();

    assert_eq!(std::env::var(key).unwrap(), "defaulted");

    std::env::remove_var(key);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shell_env_beats_both_files() {
    let key = "F_ENV_2_TEST_SHELL_C";
    std::env::set_var(key, "from_shell");

    let dir = make_tmpdir("shell");
    write_env(&dir, ".env.local", &format!("{key}=from_local\n"));
    write_env(&dir, ".env", &format!("{key}=from_env\n"));

    dotenvy::from_path(dir.join(".env.local")).unwrap();
    dotenvy::from_path(dir.join(".env")).unwrap();

    assert_eq!(std::env::var(key).unwrap(), "from_shell");

    std::env::remove_var(key);
    let _ = std::fs::remove_dir_all(&dir);
}
