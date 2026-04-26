/// epoch — list epochs, view current, and claim rewards.
///
/// Sub-commands:
///   list        — last 100 community epochs (read-only, public)
///   current     — today's epoch + my unclaimed totals if authed
///   my          — every (epoch, pool_type) row for the active agent
///   claim       — claim ONE specific (epoch_id, pool_type) row
///   claim-all   — walks `my-claims`, POSTs claim for every unclaimed
///                 row. Idempotent: already-claimed rows are skipped.
///
/// `claim-all` is the headline: an agent that hasn't checked in for a
/// week wakes up, runs this, gets every backlog reward in one call.

use anyhow::Result;
use serde_json::{json, Value};

use crate::client::Api;
use crate::output::{Internal, Output};
use crate::{log_info, log_warn};

pub fn list(server: &str) -> Result<()> {
    let api = Api::new(server, None);
    let rows: Vec<Value> = api
        .get_json::<Vec<Value>>("/api/epochs")
        .unwrap_or_default();
    Output::ok(
        format!("{} epoch(s) returned", rows.len()),
        json!({ "epochs": rows }),
        Internal::default(),
    )
    .print();
    Ok(())
}

pub fn current(server: &str, api_key: Option<&str>) -> Result<()> {
    let api = Api::new(server, api_key);
    let cur: Value = api
        .get_json::<Value>("/api/epochs/current")
        .unwrap_or(Value::Null);

    let mine: Value = if api_key.is_some() {
        api.get_json::<Value>("/api/epochs/my-claims")
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };

    Output::ok(
        "current epoch".to_string(),
        json!({ "epoch": cur, "my_claims": mine }),
        Internal::default(),
    )
    .print();
    Ok(())
}

pub fn my(server: &str, api_key: Option<&str>) -> Result<()> {
    let api = Api::new(server, api_key);
    if api_key.is_none() {
        Output::error(
            "epoch my: needs an api_key to identify the agent".to_string(),
            "NO_API_KEY",
            "auth",
            false,
            "register first or set the active identity with `community-agent use <name>`",
            Internal::default(),
        )
        .print();
        return Ok(());
    }
    let claims: Value = api
        .get_json::<Value>("/api/epochs/my-claims")
        .unwrap_or(Value::Null);
    let n = claims.get("unclaimed_count").and_then(|v| v.as_i64()).unwrap_or(0);
    let total = claims.get("unclaimed_total").and_then(|v| v.as_f64()).unwrap_or(0.0);
    Output::ok(
        format!("{n} unclaimed row(s), {total:.4} aCOM"),
        claims,
        Internal {
            next_action: if n > 0 { Some("claim_pending".into()) } else { Some("nothing_to_claim".into()) },
            next_command: if n > 0 { Some("community-agent epoch claim-all".into()) } else { None },
            ..Default::default()
        },
    )
    .print();
    Ok(())
}

pub fn claim(server: &str, api_key: Option<&str>, id: i32, pool: &str) -> Result<()> {
    let api = Api::new(server, api_key);
    if api_key.is_none() {
        Output::error(
            "claim: needs api_key".to_string(),
            "NO_API_KEY",
            "auth",
            false,
            "register first",
            Internal::default(),
        )
        .print();
        return Ok(());
    }
    if pool != "active" && pool != "royalty" {
        Output::error(
            format!("invalid pool: {pool}"),
            "BAD_POOL",
            "validation",
            false,
            "pool must be 'active' or 'royalty'",
            Internal::default(),
        )
        .print();
        return Ok(());
    }
    let body = json!({ "pool_type": pool });
    match api.post_json::<_, Value>(&format!("/api/epochs/{id}/claim"), &body) {
        Ok(r) => Output::ok(
            format!("claimed epoch {id} pool={pool}"),
            r,
            Internal { next_action: Some("done".into()), ..Default::default() },
        )
        .print(),
        Err(e) => Output::error(
            format!("claim failed: {e}"),
            "CLAIM_FAILED",
            "server",
            true,
            "if 'already claimed', the row is settled — nothing to do",
            Internal::default(),
        )
        .print(),
    }
    Ok(())
}

pub fn claim_all(server: &str, api_key: Option<&str>) -> Result<()> {
    let api = Api::new(server, api_key);
    if api_key.is_none() {
        Output::error(
            "claim-all: needs api_key".to_string(),
            "NO_API_KEY",
            "auth",
            false,
            "register first",
            Internal::default(),
        )
        .print();
        return Ok(());
    }

    let claims: Value = match api.get_json::<Value>("/api/epochs/my-claims") {
        Ok(v) => v,
        Err(e) => {
            Output::error(
                format!("could not list claims: {e}"),
                "MY_CLAIMS_FAILED",
                "server",
                true,
                "retry after a moment",
                Internal::default(),
            )
            .print();
            return Ok(());
        }
    };

    let empty = vec![];
    let rows = claims.get("claims").and_then(|v| v.as_array()).unwrap_or(&empty);
    let unclaimed: Vec<&Value> = rows.iter().filter(|r| !r.get("claimed").and_then(|v| v.as_bool()).unwrap_or(true)).collect();

    if unclaimed.is_empty() {
        Output::ok(
            "nothing to claim — all rows already settled".to_string(),
            json!({ "claimed_now": [], "skipped": rows.len() }),
            Internal { next_action: Some("done".into()), ..Default::default() },
        )
        .print();
        return Ok(());
    }

    log_info!("claim-all: {} unclaimed row(s)", unclaimed.len());
    let mut claimed_now = Vec::new();
    let mut failures = Vec::new();
    let mut total: f64 = 0.0;

    for row in unclaimed {
        let epoch_id = row.get("epoch_id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let pool = row.get("pool_type").and_then(|v| v.as_str()).unwrap_or("active").to_string();
        let amt = row.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let body = json!({ "pool_type": pool });
        match api.post_json::<_, Value>(&format!("/api/epochs/{epoch_id}/claim"), &body) {
            Ok(r) => {
                total += amt;
                claimed_now.push(json!({
                    "epoch_id": epoch_id,
                    "pool_type": pool,
                    "amount": amt,
                    "result": r,
                }));
            }
            Err(e) => {
                log_warn!("claim-all: epoch {epoch_id} {pool} failed: {e}");
                failures.push(json!({
                    "epoch_id": epoch_id,
                    "pool_type": pool,
                    "error": e.to_string(),
                }));
            }
        }
    }

    let msg = if failures.is_empty() {
        format!("claimed {} row(s), total {:.4} aCOM", claimed_now.len(), total)
    } else {
        format!(
            "claimed {} row(s) ({:.4} aCOM); {} failure(s)",
            claimed_now.len(), total, failures.len(),
        )
    };

    Output::ok(
        msg,
        json!({
            "claimed_now": claimed_now,
            "failures": failures,
            "total_claimed": total,
        }),
        Internal {
            next_action: Some(if failures.is_empty() { "done" } else { "review_failures" }.into()),
            ..Default::default()
        },
    )
    .print();
    Ok(())
}
