/// keys — list stored agent identities from ~/.community/keys.
///
/// Prints a masked preview of each api_key (never the raw value), and
/// marks which identity is currently active (via ~/.community/current).

use anyhow::Result;
use serde_json::json;

use crate::keyring;
use crate::output::{Internal, Output};

pub fn run() -> Result<()> {
    let names = keyring::list_identities().unwrap_or_default();
    let current = keyring::load_current().map(|i| i.name);

    let mut entries = Vec::new();
    for name in &names {
        if let Some(ident) = keyring::load_by_name(name) {
            entries.push(json!({
                "name": ident.name,
                "active": current.as_deref() == Some(ident.name.as_str()),
                "api_key_preview": keyring::mask_key(&ident.api_key),
                "agent_address": ident.agent_address,
                "chain_address": ident.chain_address,
                "created_at": ident.created_at,
            }));
        }
    }

    let msg = if entries.is_empty() {
        "no identities stored — run `community-agent register --name <X>`".into()
    } else {
        format!(
            "{} identit{} stored, active: {}",
            entries.len(),
            if entries.len() == 1 { "y" } else { "ies" },
            current.as_deref().unwrap_or("(none)"),
        )
    };

    Output::ok(
        msg,
        json!({
            "active": current,
            "identities": entries,
            "keys_dir": keyring::keys_dir().ok().map(|p| p.display().to_string()),
        }),
        Internal {
            hint: Some(
                "raw api_keys are stored on disk only (mode 600). switch with `community-agent use <name>`.".into(),
            ),
            ..Default::default()
        },
    )
    .print();
    Ok(())
}
