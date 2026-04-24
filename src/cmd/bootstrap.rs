/// bootstrap — install awp-wallet, init it, and register on AWP.
///
/// Pure Rust (with subprocess calls to `git`, `bash`, `awp-wallet`).
/// No Python, curl, wget, or npm required at runtime. Every step is
/// idempotent — re-running after a partial failure resumes where it
/// left off.
///
/// Three stages, each either skip-if-done or do:
///   Stage 1: awp-wallet CLI
///     skip if `awp-wallet` is on PATH; else git-clone
///     https://github.com/awp-core/awp-wallet and run its install.sh.
///   Stage 2: wallet key
///     skip if `awp-wallet receive` returns an EVM address; else
///     run `awp-wallet init` (non-interactive, generates fresh key).
///   Stage 3: AWP-chain registration
///     skip if `check_registration(address)` returns true; else call
///     `ensure_registered` which handles EIP-712 signing + gasless
///     relay + polling.

use anyhow::{Context, Result};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::awp_register;
use crate::output::{Internal, Output};
use crate::{log_debug, log_info, log_warn};

pub fn run(_server: &str, _force: bool) -> Result<()> {
    log_info!("bootstrap: starting");
    let mut stages: Vec<(String, String)> = Vec::new();

    // ── Stage 1: awp-wallet ──────────────────────────────────────
    let wallet_bin = match ensure_wallet_installed() {
        Ok(info) => {
            stages.push(("awp-wallet".into(), info.summary));
            info.bin
        }
        Err(e) => {
            bail_out(
                "AWP_WALLET_INSTALL_FAILED",
                format!("could not install awp-wallet: {e}"),
                "need `git` on PATH — install git and retry",
                "install_git_then_retry",
            );
            return Ok(());
        }
    };
    log_info!("bootstrap: wallet bin at {}", wallet_bin.display());

    // ── Stage 2: wallet init ─────────────────────────────────────
    let addr = match ensure_wallet_initialized(&wallet_bin) {
        Ok(info) => {
            stages.push(("wallet-init".into(), info.summary));
            info.address
        }
        Err(e) => {
            bail_out(
                "WALLET_INIT_FAILED",
                format!("awp-wallet init failed: {e}"),
                "inspect stderr from `awp-wallet init` directly",
                "investigate_wallet",
            );
            return Ok(());
        }
    };
    log_info!("bootstrap: wallet address {addr}");

    // ── Stage 3: AWP-chain registration ──────────────────────────
    match ensure_awp_registered(&addr) {
        Ok(info) => stages.push(("awp-register".into(), info)),
        Err(e) => {
            bail_out(
                "AWP_REGISTER_FAILED",
                format!("AWP registration failed: {e}"),
                "check api.awp.sh is reachable and awp-wallet can sign",
                "retry_after_network_check",
            );
            return Ok(());
        }
    }

    Output::ok(
        format!("bootstrap complete — wallet {addr} is registered on AWP"),
        json!({
            "wallet_address": addr,
            "wallet_bin": wallet_bin.display().to_string(),
            "stages": stages.iter().map(|(k, v)| json!({"stage": k, "state": v})).collect::<Vec<_>>(),
        }),
        Internal {
            next_action: Some("create_community_identity".into()),
            next_command: Some(
                "community-agent register --name <your_agent_name>".into(),
            ),
            hint: Some(
                "--address is auto-resolved from awp-wallet; no need to pass it explicitly.".into(),
            ),
        },
    )
    .print();
    Ok(())
}

// ── Stage 1 ─────────────────────────────────────────────────────

struct WalletInstallInfo {
    bin: PathBuf,
    summary: String,
}

fn ensure_wallet_installed() -> Result<WalletInstallInfo> {
    if let Some(bin) = find_awp_wallet() {
        return Ok(WalletInstallInfo {
            bin,
            summary: "already installed".into(),
        });
    }

    // Canonical install per awp-wallet README:
    //   git clone https://github.com/awp-core/awp-wallet ~/awp-wallet
    //   cd awp-wallet && bash install.sh --no-init
    // awp-wallet is NOT published to npm. Do not try `npm install -g`.
    let home = std::env::var("HOME").context("HOME not set")?;
    let target = Path::new(&home).join("awp-wallet");

    if !target.join(".git").exists() {
        if target.exists() {
            // Half-cloned remnant from a failed earlier run — nuke it.
            let _ = std::fs::remove_dir_all(&target);
        }
        log_info!("bootstrap: cloning awp-wallet into {}", target.display());
        let status = Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "https://github.com/awp-core/awp-wallet.git",
            ])
            .arg(&target)
            .status()
            .context("git clone awp-wallet — is git installed?")?;
        anyhow::ensure!(status.success(), "git clone exited non-zero");
    }

    log_info!("bootstrap: running awp-wallet install.sh --no-init");
    let status = Command::new("bash")
        .arg(target.join("install.sh"))
        .arg("--no-init")
        .current_dir(&target)
        .status()
        .context("bash install.sh — is bash installed?")?;
    anyhow::ensure!(status.success(), "awp-wallet install.sh exited non-zero");

    let bin = find_awp_wallet().context(
        "awp-wallet install completed but binary still not on PATH — add ~/.local/bin to PATH",
    )?;
    Ok(WalletInstallInfo {
        bin,
        summary: "installed from git".into(),
    })
}

fn find_awp_wallet() -> Option<PathBuf> {
    // Check PATH first.
    if let Ok(path) = which("awp-wallet") {
        return Some(path);
    }
    // Then well-known locations.
    let home = std::env::var("HOME").unwrap_or_default();
    for p in [
        format!("{home}/awp-wallet/bin/awp-wallet"),
        format!("{home}/.local/bin/awp-wallet"),
        format!("{home}/.npm-global/bin/awp-wallet"),
        format!("{home}/.yarn/bin/awp-wallet"),
        "/usr/local/bin/awp-wallet".to_string(),
    ] {
        let path = PathBuf::from(&p);
        if path.is_file() {
            log_debug!("bootstrap: found awp-wallet at {p}");
            return Some(path);
        }
    }
    None
}

/// Minimal which(1) lookup across $PATH.
fn which(binary: &str) -> Result<PathBuf> {
    let path = std::env::var("PATH").context("PATH not set")?;
    for dir in path.split(':') {
        let candidate = PathBuf::from(dir).join(binary);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    anyhow::bail!("{binary} not on PATH")
}

// ── Stage 2 ─────────────────────────────────────────────────────

struct WalletInitInfo {
    address: String,
    summary: String,
}

fn ensure_wallet_initialized(wallet_bin: &Path) -> Result<WalletInitInfo> {
    if let Some(addr) = read_wallet_address(wallet_bin) {
        return Ok(WalletInitInfo {
            address: addr,
            summary: "already initialized".into(),
        });
    }

    log_info!("bootstrap: initializing wallet (non-interactive)");
    let out = Command::new(wallet_bin)
        .arg("init")
        .stdin(Stdio::null())
        .output()
        .context("awp-wallet init")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("awp-wallet init failed: {stderr}");
    }

    let addr = read_wallet_address(wallet_bin).context(
        "awp-wallet init reported success but `awp-wallet receive` didn't return an address",
    )?;
    Ok(WalletInitInfo {
        address: addr,
        summary: "freshly initialized".into(),
    })
}

fn read_wallet_address(wallet_bin: &Path) -> Option<String> {
    let out = Command::new(wallet_bin).arg("receive").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);

    // awp-wallet receive emits JSON. Newer versions use "eoaAddress";
    // older ones use "address".
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(a) = v.get("eoaAddress").and_then(|x| x.as_str()) {
            if a.starts_with("0x") {
                return Some(a.to_string());
            }
        }
        if let Some(a) = v.get("address").and_then(|x| x.as_str()) {
            if a.starts_with("0x") {
                return Some(a.to_string());
            }
        }
    }
    let trimmed = stdout.trim();
    if trimmed.starts_with("0x") && trimmed.len() == 42 {
        return Some(trimmed.to_string());
    }
    log_warn!("bootstrap: couldn't parse awp-wallet receive output: {stdout}");
    None
}

// ── Stage 3 ─────────────────────────────────────────────────────

fn ensure_awp_registered(address: &str) -> Result<String> {
    match awp_register::check_registration(address)? {
        true => Ok("already registered".into()),
        false => {
            log_info!("bootstrap: address not registered, triggering ensure_registered");
            let r = awp_register::ensure_registered(address)?;
            if r.registered {
                Ok(if r.auto_registered {
                    "auto-registered via gasless relay"
                } else {
                    "registration confirmed"
                }
                .into())
            } else {
                anyhow::bail!("ensure_registered returned not-registered: {}", r.message)
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────

fn bail_out(code: &str, msg: String, hint: &str, next_action: &str) {
    Output::error(
        msg,
        code,
        "bootstrap",
        true,
        hint,
        Internal {
            next_action: Some(next_action.into()),
            ..Default::default()
        },
    )
    .print();
}
