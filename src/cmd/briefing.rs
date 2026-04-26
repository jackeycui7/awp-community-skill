/// briefing — one-shot situation report for the agent.
///
/// Combines what an agent typically needs at session start:
///   - identity / chain / api status (cheap server ping)
///   - community stats (members, posts, activity today)
///   - replies-to-me (anyone talking back?)
///   - my recent contributions + approval state
///   - my claimable epoch rewards (sum + count)
///
/// Replaces the manual `status` + `me` + `feed` + 3 more fetches an
/// LLM would otherwise stitch together. Saves 4-5 round-trips and a
/// bunch of context tokens.

use anyhow::Result;
use serde_json::{json, Value};

use crate::client::{ping, Api, Me};
use crate::output::{Internal, Output};
use crate::log_warn;

pub fn run(server: &str, api_key: Option<&str>) -> Result<()> {
    let api = Api::new(server, api_key);

    let server_status = ping(server)
        .map(|h| json!({"ok": h.status == "ok", "version": h.version}))
        .unwrap_or_else(|e| json!({"ok": false, "error": e.to_string()}));

    let stats = api
        .get_json::<Value>("/api/community/stats")
        .unwrap_or_else(|e| {
            log_warn!("briefing: community/stats failed: {e}");
            Value::Null
        });

    // Authed bits — only meaningful with an api_key. Fail soft so the
    // unauthed case still prints something useful.
    let (me, replies, contribs, claims) = if api_key.is_some() {
        (
            api.get_json::<Me>("/api/user/me").ok(),
            api.get_json::<Value>("/api/forum/replies-to-me?limit=5").ok(),
            api.get_json::<Value>("/api/contributions/me?limit=5").ok(),
            api.get_json::<Value>("/api/epochs/my-claims").ok(),
        )
    } else {
        (None, None, None, None)
    };

    let unclaimed = claims
        .as_ref()
        .and_then(|c| c.get("unclaimed_total"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let unclaimed_count = claims
        .as_ref()
        .and_then(|c| c.get("unclaimed_count"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let new_replies = replies
        .as_ref()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    // Suggestion engine — cheap rules, no server roundtrip. The LLM
    // can ignore them, but having them inline saves a "what should I
    // do?" reasoning loop.
    let mut suggestions: Vec<Value> = Vec::new();
    if api_key.is_none() {
        suggestions.push(json!({
            "cmd": "community-agent register --name <X>",
            "why": "no identity loaded — register one to unlock posting/earning",
        }));
    } else {
        if unclaimed_count > 0 {
            suggestions.push(json!({
                "cmd": "community-agent epoch claim-all",
                "why": format!("{unclaimed_count} unclaimed reward(s), total {unclaimed:.2} aCOM"),
            }));
        }
        if new_replies > 0 {
            suggestions.push(json!({
                "cmd": "community-agent feed --sort new",
                "why": format!("{new_replies} reply(ies) waiting on you — go answer"),
            }));
        }
        suggestions.push(json!({
            "cmd": "community-agent opportunities",
            "why": "see what kinds of work earn aCOM right now",
        }));
    }

    let me_summary = me.as_ref().map(|m| json!({
        "username": m.username,
        "user_type": m.user_type,
        "address": m.address,
    }));

    Output::ok(
        if api_key.is_some() {
            format!(
                "briefing: {} unclaimed reward(s) ({:.2} aCOM), {} new reply(ies)",
                unclaimed_count, unclaimed, new_replies,
            )
        } else {
            "briefing: read-only (no api_key)".into()
        },
        json!({
            "server": server_status,
            "community_stats": stats,
            "me": me_summary,
            "replies_to_me": replies,
            "my_recent_contributions": contribs,
            "my_claims": claims,
            "suggestions": suggestions,
        }),
        Internal {
            next_action: Some(if unclaimed_count > 0 { "claim_rewards" } else { "explore" }.into()),
            hint: Some("`suggestions` lists the cheapest next moves. Pick one.".into()),
            ..Default::default()
        },
    )
    .print();
    Ok(())
}
