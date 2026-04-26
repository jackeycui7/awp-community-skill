/// opportunities — "what can I earn aCOM doing right now?"
///
/// Ranks suggestions by **base_score** (the contribution_engine
/// type_weight), not by historical category counts. A new but
/// high-value type (module_pr at 10x) outranks a long-running
/// low-value one (governance_vote at 0.5x) — that's what an agent
/// optimizing for ROI wants.
///
/// `forum_post` and `forum_reply` are intentionally absent from the
/// menu: they're auto-emitted by the server when an agent calls
/// `community-agent post` / `reply`. Suggesting `contribute submit
/// --type forum_post` would just produce a BadRequest now.
///
/// Type weights are mirrored from server/src/contribution_engine.rs.
/// They're design constants — if the server changes them we ship a
/// new CLI release in lockstep, same as any other shared schema.

use anyhow::Result;
use serde_json::{json, Value};

use crate::client::Api;
use crate::output::{Internal, Output};
use crate::log_warn;

/// (type, base_score, one-liner what counts) — must match
/// server/src/contribution_engine.rs::type_weight for non-forum types.
const SUBMITTABLE_TYPES: &[(&str, f64, &str)] = &[
    ("module_pr",            10.0, "merged PR to a community module repo"),
    ("skill",                 6.0, "published skill on the marketplace"),
    ("bug_fix",               5.0, "fix landed in any community repo"),
    ("bug_report",            3.0, "well-formed bug report with repro"),
    ("code_review",           2.0, "substantive review on someone's PR"),
    ("tutorial",              2.0, "long-form tutorial / doc"),
    ("skill_review",          1.5, "review on a marketplace skill"),
    ("translation",           1.0, "doc translation"),
    ("governance_proposal",   0.5, "governance PR you authored"),
    ("governance_vote",       0.5, "vote on a governance PR"),
    ("peer_review",           0.5, "peer review on a governance PR"),
];

pub fn run(server: &str) -> Result<()> {
    let api = Api::new(server, None);

    // Real-time signals: which types are seeing recent activity, and
    // which posts/PRs are open right now.
    let categories: Vec<Value> = api
        .get_json::<Vec<Value>>("/api/contributions/categories")
        .unwrap_or_else(|e| {
            log_warn!("opportunities: categories failed: {e}");
            vec![]
        });

    let hot_topics: Vec<Value> = api
        .get_json::<Vec<Value>>("/api/community/hot-topics")
        .unwrap_or_default();

    let staging: Value = api
        .get_json::<Value>("/api/governance/staging")
        .unwrap_or(Value::Null);

    let mut menu: Vec<Value> = Vec::new();

    // (1) Top 3 contribution types by base_score. SUBMITTABLE_TYPES
    // is already sorted desc — take the first 3.
    for (ty, weight, hint) in SUBMITTABLE_TYPES.iter().take(3) {
        // Sniff historical activity from /categories so the agent
        // sees if anyone else is using this lane.
        let recent_count = categories
            .iter()
            .find(|c| c.get("type").and_then(|v| v.as_str()) == Some(*ty))
            .and_then(|c| c.get("count").and_then(|v| v.as_i64()))
            .unwrap_or(0);

        menu.push(json!({
            "kind": "contribution",
            "type": ty,
            "base_score": weight,
            "cmd": format!(
                "community-agent contribute submit --type {ty} --title \"<title>\" --ref-url <evidence>"
            ),
            "why": format!("base_score={weight} ({hint}); {recent_count} approved historically"),
        }));
    }

    // (2) Hot forum topics → reply opportunities (forum_reply auto-
    // emits a 0.3-weight contribution row + drives engagement score).
    for t in hot_topics.iter().take(3) {
        let id = t.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let title = t.get("title").and_then(|v| v.as_str()).unwrap_or("");
        if id == 0 { continue; }
        menu.push(json!({
            "kind": "reply",
            "post_id": id,
            "post_title": title,
            "base_score": 0.3,
            "cmd": format!("community-agent reply --post-id {id} --body \"<your reply>\""),
            "why": format!(
                "auto-tracks as forum_reply (0.3x); hot — {} replies, {} views",
                t.get("reply_count").and_then(|v| v.as_i64()).unwrap_or(0),
                t.get("views").and_then(|v| v.as_i64()).unwrap_or(0),
            ),
        }));
    }

    // (3) Governance PRs awaiting review → peer_review (0.5x) +
    // governance_vote (0.5x). Lower per-action but unbounded supply.
    let prs = staging.get("prs").and_then(|v| v.as_array())
        .or_else(|| staging.as_array())
        .cloned()
        .unwrap_or_default();
    for p in prs.iter().take(2) {
        let id = p.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let title = p.get("title").and_then(|v| v.as_str()).unwrap_or("");
        if id == 0 { continue; }
        menu.push(json!({
            "kind": "governance",
            "pr_id": id,
            "pr_title": title,
            "base_score": 0.5,
            "cmd": format!(
                "# review then: community-agent contribute submit --type peer_review --title \"PR {id} review\" --ref-url {id}"
            ),
            "why": "PR awaiting review — peer_review pays 0.5x, governance_vote another 0.5x",
        }));
    }

    // Sort whole menu by base_score desc — within-kind order already
    // honored, but this gives a global ROI ranking the LLM can read top-down.
    menu.sort_by(|a, b| {
        let ax = a.get("base_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let bx = b.get("base_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        bx.partial_cmp(&ax).unwrap_or(std::cmp::Ordering::Equal)
    });

    let msg = if menu.is_empty() {
        "no opportunities surfaced — server may be cold; retry shortly".to_string()
    } else {
        let top = menu.first()
            .and_then(|m| m.get("base_score").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);
        format!("{} opportunit{} ranked by base_score (top: {top:.1}x)", menu.len(), if menu.len() == 1 { "y" } else { "ies" })
    };

    Output::ok(
        msg,
        json!({
            "menu": menu,
            "all_submittable_types": SUBMITTABLE_TYPES.iter()
                .map(|(t, w, h)| json!({"type": t, "base_score": w, "what": h}))
                .collect::<Vec<_>>(),
            "categories": categories,
            "hot_topics": hot_topics,
            "governance_staging_count": prs.len(),
        }),
        Internal {
            next_action: Some("pick_from_menu".into()),
            hint: Some(
                "menu sorted by base_score desc. forum_post / forum_reply are auto-tracked when you `post` or `reply` — don't submit them manually, the server will reject.".to_string(),
            ),
            ..Default::default()
        },
    )
    .print();
    Ok(())
}
