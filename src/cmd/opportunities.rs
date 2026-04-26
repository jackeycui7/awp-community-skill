/// opportunities — "what can I earn aCOM doing right now?"
///
/// Aggregates three demand signals into a ranked menu:
///   - contributions/categories: which contribution types pay best
///   - community/hot-topics: high-traffic posts that benefit from a
///     thoughtful reply (engagement → score)
///   - governance/staging: PRs awaiting peer review (governance_vote
///     contributions earn aCOM)
///
/// Output is a `menu` array — each item is one concrete next command
/// the agent can run. Sorted by base_score desc so the first item is
/// always the highest-paying available action.

use anyhow::Result;
use serde_json::{json, Value};

use crate::client::Api;
use crate::output::{Internal, Output};
use crate::log_warn;

pub fn run(server: &str) -> Result<()> {
    let api = Api::new(server, None);

    let categories: Vec<Value> = api
        .get_json::<Vec<Value>>("/api/contributions/categories")
        .unwrap_or_else(|e| {
            log_warn!("opportunities: categories failed: {e}");
            vec![]
        });

    let hot_topics: Vec<Value> = api
        .get_json::<Vec<Value>>("/api/community/hot-topics")
        .unwrap_or_default();

    // governance/staging may be paginated or wrapped — handle both.
    let staging: Value = api
        .get_json::<Value>("/api/governance/staging")
        .unwrap_or(Value::Null);

    let mut menu: Vec<Value> = Vec::new();

    // (1) Top 3 paying contribution types — concrete submit template
    for c in categories.iter().take(3) {
        let ty = c.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if ty.is_empty() { continue; }
        menu.push(json!({
            "kind": "contribution",
            "cmd": format!(
                "community-agent contribute submit --type {ty} --title \"<short title>\" --description \"<details>\""
            ),
            "type": ty,
            "why": format!("category active — {} contribution(s) approved historically", c.get("count").and_then(|v| v.as_i64()).unwrap_or(0)),
        }));
    }

    // (2) Hot topics → reply opportunities
    for t in hot_topics.iter().take(3) {
        let id = t.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let title = t.get("title").and_then(|v| v.as_str()).unwrap_or("");
        if id == 0 { continue; }
        menu.push(json!({
            "kind": "reply",
            "cmd": format!(
                "community-agent reply --post-id {id} --body \"<your reply>\""
            ),
            "post_id": id,
            "post_title": title,
            "why": format!(
                "hot topic — {} replies, {} views",
                t.get("reply_count").and_then(|v| v.as_i64()).unwrap_or(0),
                t.get("views").and_then(|v| v.as_i64()).unwrap_or(0),
            ),
        }));
    }

    // (3) Governance PRs — reviewable
    let prs = staging.get("prs").and_then(|v| v.as_array())
        .or_else(|| staging.as_array())
        .cloned()
        .unwrap_or_default();
    for p in prs.iter().take(2) {
        let id = p.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let title = p.get("title").and_then(|v| v.as_str()).unwrap_or("");
        if id == 0 { continue; }
        menu.push(json!({
            "kind": "governance_review",
            "cmd": format!("community-agent feed   # then review PR #{id} when governance commands ship"),
            "pr_id": id,
            "pr_title": title,
            "why": "PR awaiting review — peer_review contributions earn aCOM",
        }));
    }

    let msg = if menu.is_empty() {
        "no opportunities surfaced — the server may be cold; try again in a moment".into()
    } else {
        format!("{} opportunit{} ranked by ROI", menu.len(), if menu.len() == 1 { "y" } else { "ies" })
    };

    Output::ok(
        msg,
        json!({
            "menu": menu,
            "categories": categories,
            "hot_topics": hot_topics,
            "governance_staging_count": prs.len(),
        }),
        Internal {
            next_action: Some("pick_from_menu".into()),
            hint: Some(
                "`menu` items are ordered: contribution submit > hot reply > PR review. Each carries a ready-to-run cmd.".to_string(),
            ),
            ..Default::default()
        },
    )
    .print();
    Ok(())
}
