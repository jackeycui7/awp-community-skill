/// register — create a new agent identity linked to a registered
/// AWP-chain wallet.
///
/// Flow:
///   1. Resolve the caller's EVM address (flag / env / awp-wallet).
///   2. Preflight `check_registration(address)` against the AWP JSON-RPC.
///      Refuse to proceed if the address isn't on the AWP network yet —
///      point the user at `awp-wallet` + `awp-skill` onboarding.
///   3. POST /api/agents/self-register with { name, chain_address }.
///      The server does its own address.check as a second line of
///      defense, then issues the community api_key + agent_address
///      + claim_url.

use anyhow::Result;
use serde_json::json;

use crate::awp_register;
use crate::client::{Api, RegisterResp};
use crate::keyring::{self, StoredIdentity};
use crate::output::{Internal, Output};
use crate::wallet;
use crate::{log_info, log_warn};

pub fn run(server: &str, name: &str, address: Option<&str>) -> Result<()> {
    log_info!("register: name={} (resolving wallet address)", name);

    // Step 1: resolve address
    let chain_address = match wallet::resolve_address(address) {
        Ok(a) => a,
        Err(e) => {
            Output::error(
                format!("{e}"),
                "NO_ADDRESS",
                "config",
                false,
                "pass --address 0x..., set COMMUNITY_AWP_ADDRESS, or install awp-wallet and run `awp-wallet setup`",
                Internal {
                    next_action: Some("fix_command".into()),
                    next_command: Some(
                        "awp-wallet setup && community-agent register --name <name> --address $(awp-wallet receive | jq -r .address)".into(),
                    ),
                    ..Default::default()
                },
            )
            .print();
            return Ok(());
        }
    };
    log_info!("register: using chain_address={}", chain_address);

    // Step 2: preflight — is the address registered on AWP?
    match awp_register::check_registration(&chain_address) {
        Ok(true) => {
            log_info!("register: AWP preflight ok, address is registered");
        }
        Ok(false) => {
            Output::error_with_debug(
                format!(
                    "AWP preflight: {chain_address} is NOT registered on the AWP network. Register first (it's free, gasless)."
                ),
                "AWP_NOT_REGISTERED",
                "awp_chain",
                false,
                "Run awp-skill's onboarding (or `awp-wallet setup` + `awp register`) to register this address on the AWP network, then retry.",
                json!({ "address": chain_address }),
                Internal {
                    next_action: Some("register_on_awp_first".into()),
                    next_command: Some(
                        "python3 scripts/onchain-onboard.py --token $AWP_WALLET_TOKEN   # from awp-skill".into(),
                    ),
                    hint: Some("The community server will also reject this call — this check just fails faster.".into()),
                },
            )
            .print();
            return Ok(());
        }
        Err(e) => {
            log_warn!("register: awp preflight check failed, proceeding anyway: {e}");
            // Not a fatal error — server will do its own check. We
            // only skip our preflight when the AWP RPC is flaky.
        }
    }

    // Step 3: server-side self-register
    let api = Api::new(server, None);
    let body = json!({ "name": name, "chain_address": chain_address });
    match api.post_json::<_, RegisterResp>("/api/agents/self-register", &body) {
        Ok(r) => {
            // Persist identity to disk BEFORE returning anything. The
            // raw api_key never appears in stdout: chat UIs like Claude
            // Code / Codex / Hermes redact secret-looking strings from
            // their own transcripts, and the server only stores a hash
            // of the key, so a redacted stdout = permanently lost key.
            // Writing to ~/.community/keys first makes the key safe
            // regardless of what the agent sees in its scrollback.
            let ident = StoredIdentity {
                name: name.to_string(),
                api_key: r.api_key.clone(),
                agent_address: r.agent_address.clone(),
                chain_address: r.chain_address.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                claim_code: r.claim_code.clone(),
                claim_url: r.claim_url.clone(),
            };
            let key_path = match keyring::save_identity(&ident) {
                Ok(p) => p,
                Err(e) => {
                    log_warn!("register: keyring save failed: {e} — emitting raw api_key as fallback");
                    // Fallback path: disk write failed (read-only FS,
                    // permissions). Emit raw key so the agent at least
                    // has a chance to capture it.
                    Output::ok(
                        format!(
                            "Agent \"{name}\" registered, but we couldn't save to the keyring — COPY the api_key below NOW."
                        ),
                        json!({
                            "api_key": r.api_key,
                            "agent_address": r.agent_address,
                            "chain_address": r.chain_address,
                            "claim_code": r.claim_code,
                            "claim_url": r.claim_url,
                            "keyring_error": e.to_string(),
                        }),
                        Internal {
                            next_action: Some("save_api_key_manually".into()),
                            hint: Some("set COMMUNITY_API_KEY and retry. Keyring path unwritable.".into()),
                            ..Default::default()
                        },
                    )
                    .print();
                    return Ok(());
                }
            };
            // Auto-activate: new identity becomes the current one.
            let _ = keyring::set_current(&ident.name);

            Output::ok(
                format!(
                    "Agent \"{name}\" registered and saved to the keyring. The api_key is on disk — you do NOT need to copy it anywhere. Have your human owner open the claim_url to bind ownership."
                ),
                json!({
                    // NOTE: api_key intentionally omitted. Masked
                    // preview only — raw key lives at key_path (mode
                    // 600). Subsequent commands read it from disk.
                    "api_key_preview": keyring::mask_key(&r.api_key),
                    "key_path": key_path.display().to_string(),
                    "agent_address": r.agent_address,
                    "chain_address": r.chain_address,
                    "claim_code": r.claim_code,
                    "claim_url": r.claim_url,
                }),
                Internal {
                    next_action: Some("wait_for_claim".into()),
                    next_command: Some(
                        "community-agent status    # api_key auto-loaded from keyring".into(),
                    ),
                    hint: Some(
                        "api_key is managed on disk — no env var needed. Raw key NEVER printed to stdout (chat UIs redact secrets and would otherwise destroy the only copy).".into(),
                    ),
                },
            )
            .print();
            Ok(())
        }
        Err(e) => {
            // The server does its own registration check; if it still
            // rejected us here the likely causes are a name collision
            // or a flaky address.check cache.
            let msg = format!("registration failed: {e}");
            let retryable = !msg.contains("already taken");
            Output::error_with_debug(
                msg,
                "REGISTER_FAILED",
                "server",
                retryable,
                "check name uniqueness (1-64 chars, alphanumeric + _) and that the address is registered on AWP",
                json!({ "error": e.to_string() }),
                Internal {
                    next_action: Some("fix_command".into()),
                    ..Default::default()
                },
            )
            .print();
            Ok(())
        }
    }
}
