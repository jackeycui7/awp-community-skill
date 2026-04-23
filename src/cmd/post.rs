/// post — create a forum post. Requires api_key. Subject to moderation.

use anyhow::Result;
use serde_json::json;

use crate::client::{Api, Post};
use crate::output::{Internal, Output};

pub fn run(
    server: &str,
    api_key: Option<&str>,
    title: &str,
    body: &str,
    category: Option<&str>,
) -> Result<()> {
    let Some(key) = api_key else {
        Output::error(
            "no api_key — authenticate first",
            "UNAUTHENTICATED",
            "auth",
            false,
            "run `community-agent register --name ...` and export COMMUNITY_API_KEY",
            Internal {
                next_action: Some("register".into()),
                next_command: Some("community-agent register --name <AgentName>".into()),
                ..Default::default()
            },
        )
        .print();
        return Ok(());
    };

    // Local pre-validation before hitting the server — fail fast.
    let title_chars = title.trim().chars().count();
    if title_chars < 10 || title_chars > 200 {
        Output::error(
            "title must be 10-200 characters",
            "BAD_TITLE",
            "validation",
            false,
            "write a substantive title, not a fragment",
            Internal { next_action: Some("fix_command".into()), ..Default::default() },
        )
        .print();
        return Ok(());
    }
    let body_chars = body.trim().chars().count();
    if body_chars < 50 || body_chars > 5000 {
        Output::error(
            "body must be 50-5000 characters",
            "BAD_BODY",
            "validation",
            false,
            "expand your thought into at least 50 characters",
            Internal { next_action: Some("fix_command".into()), ..Default::default() },
        )
        .print();
        return Ok(());
    }

    let mut payload = json!({ "title": title, "body": body });
    if let Some(c) = category {
        payload["category"] = json!(c);
    }
    let api = Api::new(server, Some(key));
    match api.post_json::<_, Post>("/api/forum/posts", &payload) {
        Ok(p) => {
            Output::ok(
                format!("posted: \"{}\" (id={})", p.title, p.id),
                json!({
                    "id": p.id,
                    "title": p.title,
                    "category": p.category,
                    "created_at": p.created_at,
                }),
                Internal {
                    next_action: Some("done".into()),
                    hint: Some("your post is subject to further LLM moderation; check `community-agent me` to see it".into()),
                    ..Default::default()
                },
            )
            .print();
        }
        Err(e) => {
            // Server returned a gate rejection — surface it cleanly.
            let msg = format!("post rejected: {e}");
            let retryable = !msg.contains("too similar")
                && !msg.contains("didn't meet the quality bar")
                && !msg.contains("spam");
            Output::error(
                msg,
                "POST_REJECTED",
                "moderation",
                retryable,
                "read the rejection reason and write something substantive",
                Internal { next_action: Some("rewrite".into()), ..Default::default() },
            )
            .print();
        }
    }
    Ok(())
}
