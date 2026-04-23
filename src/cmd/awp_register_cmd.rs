/// awp-register — ensure the agent is registered on-chain on the AWP
/// platform. Idempotent: if already registered (possibly via a
/// sibling worknet like Predict or Mine), we just confirm and exit.

use anyhow::Result;
use serde_json::json;

use crate::awp_register;
use crate::output::{Internal, Output};
use crate::wallet;

pub fn run(address: Option<&str>) -> Result<()> {
    let address = match wallet::resolve_address(address) {
        Ok(a) => a,
        Err(e) => {
            Output::error(
                format!("{e}"),
                "NO_ADDRESS",
                "config",
                false,
                "pass --address 0x... or export COMMUNITY_AWP_ADDRESS",
                Internal { next_action: Some("fix_command".into()), ..Default::default() },
            )
            .print();
            return Ok(());
        }
    };

    // Cheap path: just check, don't sign anything. If already on-chain
    // we're done — regardless of which worknet did the registration.
    match awp_register::check_registration(&address) {
        Ok(true) => {
            Output::ok(
                format!("{address} is already registered on AWP — nothing to do"),
                json!({
                    "address": address,
                    "registered": true,
                    "auto_registered": false,
                }),
                Internal { next_action: Some("done".into()), ..Default::default() },
            )
            .print();
            return Ok(());
        }
        Ok(false) => {
            // Not registered yet — fall through to ensure_registered.
        }
        Err(e) => {
            Output::error(
                format!("could not verify registration status: {e}"),
                "AWP_CHECK_FAILED",
                "network",
                true,
                "wait a minute and retry; the chain RPC may be flaky",
                Internal { next_action: Some("retry".into()), ..Default::default() },
            )
            .print();
            return Ok(());
        }
    }

    match awp_register::ensure_registered(&address) {
        Ok(r) => {
            Output::ok(
                r.message.clone(),
                json!({
                    "address": address,
                    "registered": r.registered,
                    "auto_registered": r.auto_registered,
                }),
                Internal { next_action: Some("done".into()), ..Default::default() },
            )
            .print();
        }
        Err(e) => {
            Output::error(
                format!("registration failed: {e}"),
                "AWP_REGISTER_FAILED",
                "chain",
                true,
                "verify awp-wallet is installed (for signing) or set COMMUNITY_AWP_PRIVATE_KEY",
                Internal { next_action: Some("retry".into()), ..Default::default() },
            )
            .print();
        }
    }
    Ok(())
}
