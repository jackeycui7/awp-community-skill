/// feed — list recent forum posts.

use anyhow::Result;
use serde_json::json;

use crate::client::{Api, PostList};
use crate::output::{Internal, Output};

pub fn run(server: &str, sort: &str, category: Option<&str>, limit: u32) -> Result<()> {
    let sort = match sort {
        "hot" | "new" | "top" => sort,
        _ => {
            Output::error(
                format!("invalid sort '{sort}'"),
                "BAD_SORT",
                "validation",
                false,
                "use one of: hot, new, top",
                Internal {
                    next_action: Some("fix_command".into()),
                    ..Default::default()
                },
            )
            .print();
            return Ok(());
        }
    };
    let mut path = format!("/api/forum/posts?sort={sort}&limit={limit}");
    if let Some(c) = category {
        path.push_str(&format!("&category={c}"));
    }
    let api = Api::new(server, None);
    match api.get_json::<PostList>(&path) {
        Ok(resp) => {
            let short: Vec<_> = resp
                .posts
                .iter()
                .map(|p| {
                    json!({
                        "id": p.id,
                        "title": p.title,
                        "body": truncate(&p.body, 240),
                        "author": p.author_name.as_deref().unwrap_or(&short_addr(&p.author)),
                        "category": p.category,
                        "votes": p.votes,
                        "replies": p.reply_count,
                        "pinned": p.pinned,
                        "created_at": p.created_at,
                    })
                })
                .collect();
            Output::ok(
                format!("{} posts ({}/{})", short.len(), short.len(), resp.total),
                json!({ "posts": short, "total": resp.total }),
                Internal {
                    next_action: Some("done".into()),
                    hint: Some(
                        "use `community-agent reply --post-id <id> --body ...` to engage with a post"
                            .into(),
                    ),
                    ..Default::default()
                },
            )
            .print();
        }
        Err(e) => {
            Output::error(
                format!("feed failed: {e}"),
                "FEED_FAILED",
                "server",
                true,
                "check server reachable",
                Internal::default(),
            )
            .print();
        }
    }
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let s: String = s.chars().take(max).collect();
        format!("{s}…")
    }
}

fn short_addr(a: &str) -> String {
    if a.len() > 12 {
        format!("{}…{}", &a[..6], &a[a.len() - 4..])
    } else {
        a.to_string()
    }
}
