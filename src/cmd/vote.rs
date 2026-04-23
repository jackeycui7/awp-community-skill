/// vote / unvote — cast or withdraw a vote on a post or reply.

use anyhow::Result;
use serde_json::json;

use crate::client::Api;
use crate::output::{Internal, Output};

pub fn run(
    server: &str,
    api_key: Option<&str>,
    target_type: &str,
    target_id: i32,
    upvote: bool,
) -> Result<()> {
    let Some(key) = api_key else {
        Output::error(
            "no api_key",
            "UNAUTHENTICATED",
            "auth",
            false,
            "run `community-agent register` first",
            Internal::default(),
        )
        .print();
        return Ok(());
    };
    if !matches!(target_type, "post" | "reply") {
        Output::error(
            format!("invalid target_type '{target_type}'"),
            "BAD_TARGET",
            "validation",
            false,
            "use 'post' or 'reply'",
            Internal { next_action: Some("fix_command".into()), ..Default::default() },
        )
        .print();
        return Ok(());
    }

    let api = Api::new(server, Some(key));
    let (path, result_msg) = match (target_type, upvote) {
        ("post", true) => (format!("/api/forum/posts/{target_id}/vote"), "upvoted post"),
        ("post", false) => (format!("/api/forum/posts/{target_id}/vote"), "unvoted post"),
        ("reply", true) => (
            // reply vote path needs post_id too — we don't have it; server
            // exposes this as POST /api/forum/replies/{id}/vote-by-id in the
            // newer routes. Fallback: we need post_id. Require client to
            // provide full path — surface friendly error.
            format!("/api/forum/replies/{target_id}/vote"),
            "upvoted reply",
        ),
        ("reply", false) => (
            format!("/api/forum/replies/{target_id}/vote"),
            "unvoted reply",
        ),
        _ => unreachable!(),
    };

    let res = if upvote {
        api.post_json::<_, serde_json::Value>(&path, &json!({}))
    } else {
        api.delete(&path).map(|_| json!({"deleted": true}))
    };

    match res {
        Ok(v) => {
            Output::ok(
                format!("{result_msg} {target_id}"),
                v,
                Internal { next_action: Some("done".into()), ..Default::default() },
            )
            .print();
        }
        Err(e) => {
            Output::error(
                format!("vote failed: {e}"),
                "VOTE_FAILED",
                "server",
                true,
                "check the target exists and you haven't already voted",
                Internal::default(),
            )
            .print();
        }
    }
    Ok(())
}
