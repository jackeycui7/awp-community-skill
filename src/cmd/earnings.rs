/// earnings — total aCOM earned and how much is still claimable.
///
/// Two numbers, one call:
///   - claimed_total:   already settled to wallet
///   - unclaimed_total: sitting in epoch_claims, ready to claim
///
/// If the agent ever wonders "did I make money this week?" this is
/// the answer in a single command. Calls /api/epochs/my-claims and
/// passes through the server's pre-computed totals.

use anyhow::Result;
use serde_json::{json, Value};

use crate::client::Api;
use crate::output::{Internal, Output};

pub fn run(server: &str, api_key: Option<&str>) -> Result<()> {
    if api_key.is_none() {
        Output::error(
            "earnings: needs api_key".to_string(),
            "NO_API_KEY",
            "auth",
            false,
            "register first",
            Internal::default(),
        )
        .print();
        return Ok(());
    }
    let api = Api::new(server, api_key);
    let claims: Value = api
        .get_json::<Value>("/api/epochs/my-claims")
        .unwrap_or(Value::Null);

    let claimed = claims.get("claimed_total").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let unclaimed = claims.get("unclaimed_total").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let unclaimed_count = claims.get("unclaimed_count").and_then(|v| v.as_i64()).unwrap_or(0);

    Output::ok(
        format!(
            "earnings: {:.4} aCOM claimed, {:.4} aCOM unclaimed across {} row(s)",
            claimed, unclaimed, unclaimed_count,
        ),
        json!({
            "claimed_total": claimed,
            "unclaimed_total": unclaimed,
            "unclaimed_count": unclaimed_count,
            "lifetime_total": claimed + unclaimed,
            "claims": claims.get("claims").cloned().unwrap_or(Value::Null),
        }),
        Internal {
            next_action: if unclaimed_count > 0 { Some("claim_pending".into()) } else { Some("done".into()) },
            next_command: if unclaimed_count > 0 { Some("community-agent epoch claim-all".into()) } else { None },
            ..Default::default()
        },
    )
    .print();
    Ok(())
}
