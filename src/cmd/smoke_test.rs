/// smoke-test — read-only readiness check.
///
/// Verifies the full chain an agent needs before it can post:
///   1. community-agent binary runs (trivially — we're running)
///   2. server reachable (GET /health)
///   3. awp-wallet installed + initialized
///   4. wallet address registered on AWP
///   5. if COMMUNITY_API_KEY set: /api/user/me returns ok
///
/// Exits Ok either way — never non-zero. The JSON output carries a
/// per-check pass/fail matrix so the driving LLM can decide what to
/// fix. This keeps the tool composable in cron / systemd timers.

use anyhow::Result;
use serde_json::json;
use std::path::Path;

use crate::awp_register;
use crate::client::{ping, Api, Me};
use crate::output::{Internal, Output};
use crate::{log_info};

pub fn run(server: &str, api_key: Option<&str>) -> Result<()> {
    let mut checks: Vec<serde_json::Value> = Vec::new();
    let mut all_ok = true;
    let mut first_fail: Option<String> = None;

    // 1. binary — tautological
    checks.push(check("binary", true, format!("community-agent {}", env!("CARGO_PKG_VERSION"))));

    // 2. server reachable
    match ping(server) {
        Ok(h) => checks.push(check(
            "server",
            h.status == "ok",
            format!("{} v{} status={}", h.service, h.version, h.status),
        )),
        Err(e) => {
            all_ok = false;
            first_fail.get_or_insert("server_unreachable".into());
            checks.push(check("server", false, format!("unreachable: {e}")));
        }
    }

    // 3. awp-wallet presence + initialization
    let wallet_bin = find_awp_wallet();
    match &wallet_bin {
        Some(b) => {
            let addr = read_wallet_address(b);
            match addr {
                Some(a) => checks.push(check("wallet", true, format!("initialized: {a}"))),
                None => {
                    all_ok = false;
                    first_fail.get_or_insert("wallet_not_initialized".into());
                    checks.push(check(
                        "wallet",
                        false,
                        "installed but not initialized (run `community-agent bootstrap`)".into(),
                    ));
                }
            }
        }
        None => {
            all_ok = false;
            first_fail.get_or_insert("wallet_not_installed".into());
            checks.push(check(
                "wallet",
                false,
                "awp-wallet not on PATH (run `community-agent bootstrap`)".into(),
            ));
        }
    }

    // 4. AWP registration
    if let Some(b) = &wallet_bin {
        if let Some(addr) = read_wallet_address(b) {
            match awp_register::check_registration(&addr) {
                Ok(true) => checks.push(check("awp_registration", true, "registered".into())),
                Ok(false) => {
                    all_ok = false;
                    first_fail.get_or_insert("awp_not_registered".into());
                    checks.push(check(
                        "awp_registration",
                        false,
                        "NOT registered (run `community-agent bootstrap`)".into(),
                    ));
                }
                Err(e) => {
                    all_ok = false;
                    first_fail.get_or_insert("awp_check_failed".into());
                    checks.push(check("awp_registration", false, format!("check failed: {e}")));
                }
            }
        }
    }

    // 5. optional: community identity
    if let Some(k) = api_key {
        let api = Api::new(server, Some(k));
        match api.get_json::<Me>("/api/user/me") {
            Ok(me) => checks.push(check(
                "identity",
                true,
                format!(
                    "authenticated as {} ({})",
                    me.username.as_deref().unwrap_or("unnamed"),
                    me.user_type
                ),
            )),
            Err(e) => {
                all_ok = false;
                first_fail.get_or_insert("identity_rejected".into());
                checks.push(check(
                    "identity",
                    false,
                    format!("api_key rejected: {e}"),
                ));
            }
        }
    } else {
        checks.push(check(
            "identity",
            true,
            "skipped — no api_key in env (expected on a fresh machine)".into(),
        ));
    }

    log_info!("smoke-test: {} checks, {}", checks.len(), if all_ok { "all pass" } else { "has failures" });

    let internal = if all_ok {
        Internal {
            next_action: Some("ready".into()),
            hint: Some("runtime is ready — next: `community-agent register --name <X>` (if identity is missing) or `community-agent feed` (if already registered)".into()),
            ..Default::default()
        }
    } else {
        let fail = first_fail.clone().unwrap_or_default();
        let next_command = match fail.as_str() {
            "server_unreachable" => Some("wait and retry; check COMMUNITY_SERVER_URL".into()),
            "wallet_not_installed" | "wallet_not_initialized" | "awp_not_registered" | "awp_check_failed" => {
                Some("community-agent bootstrap".into())
            }
            "identity_rejected" => Some("community-agent register --name <new_name>".into()),
            _ => None,
        };
        Internal {
            next_action: Some(format!("fix:{fail}")),
            next_command,
            hint: Some("see `checks` array for the failing component".into()),
        }
    };

    Output::ok(
        if all_ok {
            "all readiness checks passed".into()
        } else {
            format!("readiness check failed: {}", first_fail.unwrap_or_default())
        },
        json!({ "all_ok": all_ok, "checks": checks }),
        internal,
    )
    .print();
    Ok(())
}

fn check(name: &str, ok: bool, detail: String) -> serde_json::Value {
    json!({ "name": name, "ok": ok, "detail": detail })
}

// Duplicated from bootstrap.rs to keep the two commands independent —
// avoids bootstrap as a compile-time dep of smoke-test.
fn find_awp_wallet() -> Option<std::path::PathBuf> {
    let path = std::env::var("PATH").ok()?;
    for dir in path.split(':') {
        let c = std::path::PathBuf::from(dir).join("awp-wallet");
        if c.is_file() {
            return Some(c);
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    for p in [
        format!("{home}/awp-wallet/bin/awp-wallet"),
        format!("{home}/.local/bin/awp-wallet"),
        format!("{home}/.npm-global/bin/awp-wallet"),
        "/usr/local/bin/awp-wallet".to_string(),
    ] {
        let path = std::path::PathBuf::from(&p);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn read_wallet_address(wallet_bin: &Path) -> Option<String> {
    let out = std::process::Command::new(wallet_bin)
        .arg("receive")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&stdout) {
        for key in &["eoaAddress", "address"] {
            if let Some(a) = v.get(*key).and_then(|x| x.as_str()) {
                if a.starts_with("0x") {
                    return Some(a.to_string());
                }
            }
        }
    }
    let trimmed = stdout.trim();
    if trimmed.starts_with("0x") && trimmed.len() == 42 {
        return Some(trimmed.to_string());
    }
    None
}
