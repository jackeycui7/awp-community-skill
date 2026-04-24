/// use — switch active identity by writing <name> to ~/.community/current.

use anyhow::Result;
use serde_json::json;

use crate::keyring;
use crate::output::{Internal, Output};

pub fn run(name: &str) -> Result<()> {
    let ident = match keyring::load_by_name(name) {
        Some(i) => i,
        None => {
            let available = keyring::list_identities().unwrap_or_default();
            Output::error(
                format!("no identity named \"{name}\" in the keyring"),
                "IDENTITY_NOT_FOUND",
                "keyring",
                false,
                "run `community-agent keys` to list available identities, or `community-agent register --name <X>` to create one",
                Internal {
                    next_action: Some("fix_command".into()),
                    hint: Some(format!("available: {}", if available.is_empty() { "(none)".into() } else { available.join(", ") })),
                    ..Default::default()
                },
            )
            .print();
            return Ok(());
        }
    };

    keyring::set_current(&ident.name)?;

    Output::ok(
        format!("active identity is now \"{}\"", ident.name),
        json!({
            "name": ident.name,
            "agent_address": ident.agent_address,
            "chain_address": ident.chain_address,
            "api_key_preview": keyring::mask_key(&ident.api_key),
        }),
        Internal {
            next_action: Some("ready".into()),
            next_command: Some("community-agent status".into()),
            hint: Some("subsequent commands auto-read the api_key from disk; no env var needed.".into()),
        },
    )
    .print();
    Ok(())
}
