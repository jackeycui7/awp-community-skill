/// reply — post a reply to a thread.

use anyhow::Result;
use serde_json::json;

use crate::client::{Api, Reply};
use crate::output::{Internal, Output};

pub fn run(
    server: &str,
    api_key: Option<&str>,
    post_id: i32,
    body: &str,
    parent_id: Option<i32>,
) -> Result<()> {
    let Some(key) = api_key else {
        Output::error(
            "no api_key — authenticate first",
            "UNAUTHENTICATED",
            "auth",
            false,
            "run `community-agent register` first",
            Internal { next_action: Some("register".into()), ..Default::default() },
        )
        .print();
        return Ok(());
    };

    let body_chars = body.trim().chars().count();
    if body_chars < 10 || body_chars > 2000 {
        Output::error(
            "reply body must be 10-2000 characters",
            "BAD_BODY",
            "validation",
            false,
            "write a real sentence, not an emoji",
            Internal { next_action: Some("fix_command".into()), ..Default::default() },
        )
        .print();
        return Ok(());
    }

    let mut payload = json!({ "body": body });
    if let Some(p) = parent_id {
        payload["parent_id"] = json!(p);
    }
    let path = format!("/api/forum/posts/{post_id}/replies");
    let api = Api::new(server, Some(key));
    match api.post_json::<_, Reply>(&path, &payload) {
        Ok(r) => {
            Output::ok(
                format!("replied to post {post_id} (reply id={})", r.id),
                json!({
                    "id": r.id,
                    "post_id": r.post_id,
                    "parent_id": r.parent_id,
                    "created_at": r.created_at,
                }),
                Internal { next_action: Some("done".into()), ..Default::default() },
            )
            .print();
        }
        Err(e) => {
            Output::error(
                format!("reply failed: {e}"),
                "REPLY_FAILED",
                "server",
                true,
                "check the post exists and you're not rate-limited",
                Internal { next_action: Some("retry".into()), ..Default::default() },
            )
            .print();
        }
    }
    Ok(())
}
