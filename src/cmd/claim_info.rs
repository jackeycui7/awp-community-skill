/// claim-info — inspect a claim code's status.

use anyhow::Result;
use serde_json::json;

use crate::client::{Api, ClaimInfo};
use crate::output::{Internal, Output};

pub fn run(server: &str, code: &str) -> Result<()> {
    let api = Api::new(server, None);
    let path = format!("/api/agents/claim-info?code={}", urlencoding(code));
    match api.get_json::<ClaimInfo>(&path) {
        Ok(info) => {
            let state = if info.claimed.unwrap_or(false) {
                "CLAIMED"
            } else if info.expired.unwrap_or(false) {
                "EXPIRED"
            } else if info.valid {
                "PENDING"
            } else {
                "UNKNOWN"
            };
            Output::ok(
                format!("claim {} — state: {state}", truncate_code(code)),
                json!({
                    "state": state,
                    "valid": info.valid,
                    "agent_name": info.agent_name,
                    "expired": info.expired,
                    "claimed": info.claimed,
                    "sponsor": info.sponsor,
                }),
                Internal {
                    next_action: Some(match state {
                        "PENDING" => "wait_for_claim",
                        "CLAIMED" => "done",
                        _ => "regenerate",
                    }.into()),
                    ..Default::default()
                },
            )
            .print();
        }
        Err(e) => {
            Output::error(
                format!("claim-info failed: {e}"),
                "CLAIM_INFO_FAILED",
                "server",
                true,
                "check the claim code is correct and hasn't been deleted",
                Internal::default(),
            )
            .print();
        }
    }
    Ok(())
}

fn urlencoding(s: &str) -> String {
    // tiny manual URL encoder for safety — claim codes are alphanumeric so
    // this is mostly a no-op, but guard anyway.
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') {
            out.push(c);
        } else {
            for b in c.to_string().as_bytes() {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

fn truncate_code(c: &str) -> String {
    if c.len() > 20 {
        format!("{}…", &c[..20])
    } else {
        c.to_string()
    }
}
