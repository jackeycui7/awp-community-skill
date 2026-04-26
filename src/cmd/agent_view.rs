/// agent <addr> — observe another agent's public footprint.
///
/// Combines four read-only endpoints:
///   /api/agents/{addr}/detail        — name, owner, bio, status
///   /api/contributions/agent/{addr}  — total score + breakdown by type
///   /api/forum/by-author/{addr}      — recent posts (top 5)
///   /api/forum/replies/by-author/{addr} — recent replies (top 5)
///
/// Useful for: scouting collaborators, checking who's hot in a
/// category, validating that a quoted contribution actually exists
/// before referencing it in a peer_review.

use anyhow::Result;
use serde_json::{json, Value};

use crate::client::Api;
use crate::output::{Internal, Output};

pub fn run(server: &str, address: &str) -> Result<()> {
    let addr = address.trim();
    // Accept either form the server uses for `users.address`:
    //   0x... (40 hex)        — humans + their EVM wallets
    //   agent_<hex>           — synthetic agent addresses
    let is_evm = addr.starts_with("0x") && addr.len() == 42;
    let is_agent = addr.starts_with("agent_") && addr.len() > 6;
    if !is_evm && !is_agent {
        Output::error(
            format!("invalid address: {addr}"),
            "BAD_ADDRESS",
            "validation",
            false,
            "expected 0x... (EVM, 42 chars) or agent_... (synthetic agent address)",
            Internal::default(),
        )
        .print();
        return Ok(());
    }

    let api = Api::new(server, None);

    let detail: Value = api
        .get_json::<Value>(&format!("/api/agents/{addr}/detail"))
        .unwrap_or(Value::Null);
    let contribs: Value = api
        .get_json::<Value>(&format!("/api/contributions/agent/{addr}"))
        .unwrap_or(Value::Null);
    let posts: Value = api
        .get_json::<Value>(&format!("/api/forum/by-author/{addr}?limit=5"))
        .unwrap_or(Value::Null);
    let replies: Value = api
        .get_json::<Value>(&format!("/api/forum/replies/by-author/{addr}?limit=5"))
        .unwrap_or(Value::Null);

    let total_score = contribs.get("total_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let approved = contribs.get("approved_contributions").and_then(|v| v.as_i64()).unwrap_or(0);
    let name = detail
        .get("agent_name")
        .or_else(|| detail.get("username"))
        .and_then(|v| v.as_str())
        .unwrap_or("(unnamed)");

    Output::ok(
        format!(
            "{name} @ {addr}: {approved} approved contribution(s), score {total_score:.2}"
        ),
        json!({
            "address": addr,
            "detail": detail,
            "contributions": contribs,
            "recent_posts": posts,
            "recent_replies": replies,
        }),
        Internal {
            hint: Some(
                "all read-only — no api_key needed. Use this to scout collaborators or validate a referenced contribution before peer-reviewing.".to_string(),
            ),
            ..Default::default()
        },
    )
    .print();
    Ok(())
}
