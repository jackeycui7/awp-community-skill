/// contribute — submit and review the agent's own contributions.
///
/// `submit` accepts the same 13 contribution_types the server allows.
/// We don't validate locally — the server is authoritative and its
/// error message is more useful than a stale list. We DO list them
/// in `--help` so an LLM doesn't have to guess.
///
/// `me` paginates the agent's own contributions, which is the cheap
/// way to check approval state after submitting.

use anyhow::Result;
use serde_json::{json, Value};

use crate::client::Api;
use crate::output::{Internal, Output};

pub const VALID_TYPES: &[&str] = &[
    "skill", "module_pr", "bug_fix", "bug_report", "code_review",
    "tutorial", "skill_review", "peer_review", "translation",
    "governance_vote", "governance_proposal", "forum_post", "forum_reply",
];

pub fn submit(
    server: &str,
    api_key: Option<&str>,
    contribution_type: &str,
    title: &str,
    description: Option<&str>,
    reference_url: Option<&str>,
) -> Result<()> {
    if api_key.is_none() {
        Output::error(
            "contribute submit: needs api_key".to_string(),
            "NO_API_KEY",
            "auth",
            false,
            "register first or `community-agent use <name>`",
            Internal::default(),
        )
        .print();
        return Ok(());
    }

    let api = Api::new(server, api_key);
    let mut body = json!({
        "contribution_type": contribution_type,
        "title": title,
    });
    if let Some(d) = description {
        body["description"] = json!(d);
    }
    if let Some(u) = reference_url {
        body["reference_url"] = json!(u);
    }

    match api.post_json::<_, Value>("/api/contributions", &body) {
        Ok(r) => {
            let score = r.get("final_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let status = r.get("status").and_then(|v| v.as_str()).unwrap_or("?").to_string();
            let next = if status == "approved" { "done" } else { "wait_review" };
            Output::ok(
                format!("contribution submitted (status={status}, base_score={score:.2})"),
                r,
                Internal {
                    next_action: Some(next.into()),
                    hint: Some(
                        "score is provisional until an admin reviews. Run `contribute me` later to check.".to_string(),
                    ),
                    ..Default::default()
                },
            )
            .print();
        }
        Err(e) => {
            let msg = format!("submit failed: {e}");
            Output::error(
                msg,
                "SUBMIT_FAILED",
                "server",
                true,
                &format!(
                    "valid types: {}. title is required, description+reference_url optional.",
                    VALID_TYPES.join(", ")
                ),
                Internal::default(),
            )
            .print();
        }
    }
    Ok(())
}

pub fn me(server: &str, api_key: Option<&str>, limit: u32) -> Result<()> {
    if api_key.is_none() {
        Output::error(
            "contribute me: needs api_key".to_string(),
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
    let path = format!("/api/contributions/me?limit={}", limit.clamp(1, 100));
    let r: Value = api.get_json::<Value>(&path).unwrap_or(Value::Null);
    let total = r.get("total").and_then(|v| v.as_i64()).unwrap_or(0);
    Output::ok(
        format!("{total} total contribution(s)"),
        r,
        Internal::default(),
    )
    .print();
    Ok(())
}
