use assert_cmd::Command;
use predicates::prelude::*;

const TEST_SERVER: &str = "http://storm-front:3002";
const TEST_TOKEN: &str = "tNub244N_MBnBqhLH7PE2fjwQD9w2w69t6f3uCrPM";

fn plnk() -> Command {
    let mut cmd = Command::cargo_bin("plnk").unwrap();
    // Point config to a guaranteed non-existent path so tests
    // never read the user's real ~/.config/planka/config.toml
    cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
    cmd
}

fn plnk_authed() -> Command {
    let mut cmd = plnk();
    cmd.env("PLANKA_SERVER", TEST_SERVER);
    cmd.env("PLANKA_TOKEN", TEST_TOKEN);
    cmd
}

// ─── Help ────────────────────────────────────────────────────────────

#[test]
fn help_shows_auth_subcommand() {
    plnk()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("auth"));
}

#[test]
fn auth_help_shows_all_subcommands() {
    plnk()
        .args(["auth", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("login"))
        .stdout(predicate::str::contains("token"))
        .stdout(predicate::str::contains("whoami"))
        .stdout(predicate::str::contains("logout"))
        .stdout(predicate::str::contains("status"));
}

#[test]
fn auth_login_help_shows_all_flags() {
    plnk()
        .args(["auth", "login", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--server"))
        .stdout(predicate::str::contains("--email"))
        .stdout(predicate::str::contains("--password"));
}

// ─── Missing auth → exit 3 ──────────────────────────────────────────

#[test]
fn whoami_no_creds_exits_3() {
    plnk()
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "whoami"])
        .assert()
        .failure()
        .code(3);
}

#[test]
fn whoami_no_creds_json_structured_error() {
    plnk()
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "whoami", "--output", "json"])
        .assert()
        .failure()
        .code(3)
        .stdout(predicate::str::contains("\"success\": false"))
        .stdout(predicate::str::contains("AuthenticationFailed"));
}

#[test]
fn partial_flags_exits_3() {
    plnk()
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "whoami", "--server", TEST_SERVER])
        .assert()
        .failure()
        .code(3);
}

// ─── Whoami (live server) ────────────────────────────────────────────

#[test]
fn whoami_table_shows_user() {
    plnk_authed()
        .args(["auth", "whoami"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Claude"))
        .stdout(predicate::str::contains("projectOwner"));
}

#[test]
fn whoami_json_envelope() {
    let output = plnk_authed()
        .args(["auth", "whoami", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["name"], "Claude");
    assert_eq!(json["data"]["role"], "projectOwner");
    assert!(json["data"]["id"].is_string());
}

#[test]
fn whoami_markdown_format() {
    plnk_authed()
        .args(["auth", "whoami", "--output", "markdown"])
        .assert()
        .success()
        .stdout(predicate::str::contains("**Name:** Claude"))
        .stdout(predicate::str::contains("**Role:** projectOwner"));
}

#[test]
fn whoami_invalid_token_exits_3() {
    plnk()
        .env("PLANKA_SERVER", TEST_SERVER)
        .env("PLANKA_TOKEN", "invalid-token-12345")
        .args(["auth", "whoami"])
        .assert()
        .failure()
        .code(3);
}

// ─── Status (live server) ────────────────────────────────────────────

#[test]
fn status_table_shows_server_and_source() {
    plnk_authed()
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Server:"))
        .stdout(predicate::str::contains("Source:"))
        .stdout(predicate::str::contains("User: Claude"));
}

#[test]
fn status_json_envelope() {
    let output = plnk_authed()
        .args(["auth", "status", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["authenticated"], true);
    // Clap's `env` attribute populates the flag from env vars, so
    // resolve_credentials sees them as flags, not env vars.
    assert_eq!(json["data"]["source"], "CLI flags");
    assert!(
        json["data"]["server"]
            .as_str()
            .unwrap()
            .contains("storm-front")
    );
}

#[test]
fn status_no_creds_shows_unauthenticated() {
    plnk()
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "status"])
        .assert()
        .success() // status is informational, always exits 0
        .stdout(predicate::str::contains("Not authenticated"));
}

#[test]
fn status_no_creds_json() {
    let output = plnk()
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "status", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["authenticated"], false);
}

// ─── Logout ──────────────────────────────────────────────────────────

#[test]
fn logout_succeeds() {
    // Use a temp config dir so we don't clobber the user's real config
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");

    // Write a dummy config first
    std::fs::write(
        &config_path,
        "server = \"http://example.com\"\ntoken = \"abc\"",
    )
    .unwrap();

    plnk()
        .env("PLANKA_CONFIG", config_path.to_str().unwrap())
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "logout"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Logged out"));

    assert!(!config_path.exists(), "config file should be deleted");
}

#[test]
fn logout_json_output() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    std::fs::write(
        &config_path,
        "server = \"http://example.com\"\ntoken = \"abc\"",
    )
    .unwrap();

    let output = plnk()
        .env("PLANKA_CONFIG", config_path.to_str().unwrap())
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "logout", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], true);
}

// ─── Token set ───────────────────────────────────────────────────────

#[test]
fn token_set_writes_config() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");

    plnk()
        .env("PLANKA_CONFIG", config_path.to_str().unwrap())
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args([
            "auth",
            "token",
            "set",
            "my-test-token",
            "--server",
            "http://example.com",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Token saved"));

    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("my-test-token"));
    assert!(content.contains("http://example.com"));
}

#[test]
fn token_set_no_server_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    // No existing config, no env, no flag → should fail

    plnk()
        .env("PLANKA_CONFIG", config_path.to_str().unwrap())
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "token", "set", "some-token"])
        .assert()
        .failure()
        .code(3);
}

#[test]
fn token_set_uses_existing_server() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");

    // Write a config with a server already set
    std::fs::write(
        &config_path,
        "server = \"http://existing-server.com\"\ntoken = \"old-token\"",
    )
    .unwrap();

    plnk()
        .env("PLANKA_CONFIG", config_path.to_str().unwrap())
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "token", "set", "new-token"])
        .assert()
        .success();

    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("new-token"));
    assert!(content.contains("existing-server.com"));
}

// ─── Token set → whoami flow (live server) ───────────────────────────

#[test]
fn token_set_then_whoami_flow() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");

    // Set token via CLI
    plnk()
        .env("PLANKA_CONFIG", config_path.to_str().unwrap())
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "token", "set", TEST_TOKEN, "--server", TEST_SERVER])
        .assert()
        .success();

    // Now whoami should work reading from config
    plnk()
        .env("PLANKA_CONFIG", config_path.to_str().unwrap())
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "whoami"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Claude"));

    // Status should show config file source
    plnk()
        .env("PLANKA_CONFIG", config_path.to_str().unwrap())
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("config file"));

    // Logout
    plnk()
        .env("PLANKA_CONFIG", config_path.to_str().unwrap())
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "logout"])
        .assert()
        .success();

    // Whoami should now fail
    plnk()
        .env("PLANKA_CONFIG", config_path.to_str().unwrap())
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "whoami"])
        .assert()
        .failure()
        .code(3);
}

// ─── Verbosity ───────────────────────────────────────────────────────

#[test]
fn verbose_logs_to_stderr() {
    plnk_authed()
        .args(["auth", "whoami", "-vv"])
        .assert()
        .success()
        .stderr(predicate::str::contains("GET"));
}

#[test]
fn quiet_suppresses_logs() {
    plnk_authed()
        .args(["auth", "whoami", "--quiet"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

// ─── Version ─────────────────────────────────────────────────────────

#[test]
fn version_output() {
    plnk()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("plnk 0.1.0"));
}
