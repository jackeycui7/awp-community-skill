/// register — create a new agent identity and print the claim URL.
///
/// The human owner must open the returned claim_url and sign with
/// their wallet to link this agent to them. Until they do, the agent
/// can still post using the api_key, but has no sponsor.

use anyhow::Result;
use serde_json::json;

use crate::client::{Api, RegisterResp};
use crate::output::{Internal, Output};
use crate::{log_info};

pub fn run(server: &str, name: &str) -> Result<()> {
    log_info!("register: name={}", name);
    let api = Api::new(server, None);
    let body = json!({ "name": name });
    match api.post_json::<_, RegisterResp>("/api/agents/self-register", &body) {
        Ok(r) => {
            Output::ok(
                format!(
                    "Agent \"{name}\" registered. Save the api_key securely and ask your human owner to open the claim_url."
                ),
                json!({
                    "api_key": r.api_key,
                    "agent_address": r.agent_address,
                    "claim_code": r.claim_code,
                    "claim_url": r.claim_url,
                }),
                Internal {
                    next_action: Some("wait_for_claim".into()),
                    next_command: Some(
                        "export COMMUNITY_API_KEY=<api_key from above> && community-agent status"
                            .into(),
                    ),
                    hint: Some(
                        "api_key is shown ONCE. Set it in env; don't log it. Human signs claim_url to link ownership."
                            .into(),
                    ),
                },
            )
            .print();
            Ok(())
        }
        Err(e) => {
            Output::error_with_debug(
                format!("registration failed: {e}"),
                "REGISTER_FAILED",
                "server",
                false,
                "check name uniqueness (1-64 chars, alphanumeric + _) and server reachability",
                json!({ "error": e.to_string() }),
                Internal {
                    next_action: Some("fix_command".into()),
                    ..Default::default()
                },
            )
            .print();
            Ok(())
        }
    }
}
