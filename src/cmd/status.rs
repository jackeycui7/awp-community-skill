/// status — health check + auth state.

use anyhow::Result;
use serde_json::json;

use crate::client::{ping, Api, Me};
use crate::output::{Internal, Output};
use crate::{log_info};

pub fn run(server: &str, api_key: Option<&str>) -> Result<()> {
    log_info!("status: pinging {}", server);
    let health = match ping(server) {
        Ok(h) => h,
        Err(e) => {
            Output::error_with_debug(
                format!("server unreachable: {e}"),
                "SERVER_DOWN",
                "network",
                true,
                "wait a minute and try again, or check COMMUNITY_SERVER_URL",
                json!({ "error": e.to_string() }),
                Internal {
                    next_action: Some("retry".into()),
                    ..Default::default()
                },
            )
            .print();
            return Ok(());
        }
    };

    // If api_key is set, try /api/user/me
    let me = api_key
        .and_then(|k| Api::new(server, Some(k)).get_json::<Me>("/api/user/me").ok());

    let authed = me.is_some();
    let data = json!({
        "server": {
            "service": health.service,
            "version": health.version,
            "status": health.status,
        },
        "auth": {
            "api_key_provided": api_key.is_some(),
            "authenticated": authed,
            "me": me.as_ref().map(|m| json!({
                "address": m.address,
                "username": m.username,
                "user_type": m.user_type,
                "is_admin": m.is_admin,
            })),
        },
    });

    let msg = if authed {
        format!(
            "server ok · authenticated as {} ({})",
            me.as_ref().and_then(|m| m.username.as_deref()).unwrap_or("unnamed"),
            me.as_ref().map(|m| m.user_type.as_str()).unwrap_or("?")
        )
    } else if api_key.is_some() {
        "server ok · api_key rejected (expired or revoked?)".into()
    } else {
        "server ok · no api_key (run `community-agent register` first)".into()
    };

    let next = if !authed {
        Some("run `community-agent register --name YourAgent` if you don't have an api_key".into())
    } else {
        None
    };

    Output::ok(
        msg,
        data,
        Internal {
            next_action: Some(if authed { "done" } else { "register" }.into()),
            hint: next,
            ..Default::default()
        },
    )
    .print();
    Ok(())
}
