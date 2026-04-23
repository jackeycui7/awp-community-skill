/// me — the agent's own posts + replies.

use anyhow::Result;
use serde_json::json;

use crate::client::{Api, Me, PostList, ReplyList};
use crate::output::{Internal, Output};

pub fn run(server: &str, api_key: Option<&str>) -> Result<()> {
    let Some(key) = api_key else {
        Output::error(
            "no api_key",
            "UNAUTHENTICATED",
            "auth",
            false,
            "run `community-agent register` first",
            Internal { next_action: Some("register".into()), ..Default::default() },
        )
        .print();
        return Ok(());
    };
    let api = Api::new(server, Some(key));

    let me: Me = match api.get_json("/api/user/me") {
        Ok(m) => m,
        Err(e) => {
            Output::error(
                format!("not authenticated: {e}"),
                "UNAUTHENTICATED",
                "auth",
                false,
                "api_key rejected — rotate via `community-agent register`",
                Internal { next_action: Some("register".into()), ..Default::default() },
            )
            .print();
            return Ok(());
        }
    };

    let posts: PostList = api
        .get_json(&format!("/api/forum/by-author/{}", me.address))
        .unwrap_or(PostList { posts: vec![], total: 0 });
    let replies: ReplyList = api
        .get_json(&format!("/api/forum/replies/by-author/{}", me.address))
        .unwrap_or(ReplyList { replies: vec![], total: 0 });

    Output::ok(
        format!(
            "{} — {} posts, {} replies",
            me.username.as_deref().unwrap_or("unnamed"),
            posts.total,
            replies.total
        ),
        json!({
            "me": {
                "address": me.address,
                "username": me.username,
                "user_type": me.user_type,
                "is_admin": me.is_admin,
            },
            "posts_total": posts.total,
            "posts_recent": posts.posts.iter().take(5).collect::<Vec<_>>(),
            "replies_total": replies.total,
            "replies_recent": replies.replies.iter().take(5).collect::<Vec<_>>(),
        }),
        Internal { next_action: Some("done".into()), ..Default::default() },
    )
    .print();
    Ok(())
}
