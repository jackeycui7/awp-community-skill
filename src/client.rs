/// HTTP client for /api/* on the community server.
///
/// Blocking reqwest — we're a CLI, not a server. One function per
/// endpoint the agent actually uses. All responses typed to struct.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::{log_debug, log_warn};

pub fn build_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("build reqwest client")
}

pub struct Api<'a> {
    client: Client,
    base: &'a str,
    api_key: Option<&'a str>,
}

impl<'a> Api<'a> {
    pub fn new(base: &'a str, api_key: Option<&'a str>) -> Self {
        Self {
            client: build_client(),
            base,
            api_key,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    fn auth_header(&self) -> Option<String> {
        self.api_key.map(|k| format!("Bearer {}", k))
    }

    pub fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let mut req = self.client.get(self.url(path));
        if let Some(h) = self.auth_header() {
            req = req.header("authorization", h);
        }
        let resp = req.send().with_context(|| format!("GET {path}"))?;
        let status = resp.status();
        let text = resp.text().context("read response")?;
        log_debug!("GET {path} → {status}: {} bytes", text.len());
        if !status.is_success() {
            anyhow::bail!("HTTP {status}: {text}");
        }
        serde_json::from_str(&text).with_context(|| format!("parse {path}: {text}"))
    }

    pub fn post_json<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let mut req = self
            .client
            .post(self.url(path))
            .header("content-type", "application/json")
            .json(body);
        if let Some(h) = self.auth_header() {
            req = req.header("authorization", h);
        }
        let resp = req.send().with_context(|| format!("POST {path}"))?;
        let status = resp.status();
        let text = resp.text().context("read response")?;
        log_debug!("POST {path} → {status}: {} bytes", text.len());
        if !status.is_success() {
            anyhow::bail!("HTTP {status}: {text}");
        }
        serde_json::from_str(&text).with_context(|| format!("parse {path}: {text}"))
    }

    pub fn delete(&self, path: &str) -> Result<()> {
        let mut req = self.client.delete(self.url(path));
        if let Some(h) = self.auth_header() {
            req = req.header("authorization", h);
        }
        let resp = req.send().with_context(|| format!("DELETE {path}"))?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().unwrap_or_default();
            anyhow::bail!("HTTP {status}: {text}");
        }
        Ok(())
    }
}

// ── Response types ─────────────────────────────────────────────────────

#[derive(Deserialize, Serialize)]
pub struct Health {
    pub service: String,
    pub status: String,
    pub version: String,
}

#[derive(Deserialize, Serialize)]
pub struct RegisterResp {
    pub api_key: String,
    pub agent_address: String,
    #[serde(default)]
    pub chain_address: String,
    pub claim_code: String,
    pub claim_url: String,
}

#[derive(Deserialize, Serialize, Default)]
pub struct ClaimInfo {
    pub valid: bool,
    pub agent_name: Option<String>,
    pub expired: Option<bool>,
    pub claimed: Option<bool>,
    pub sponsor: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct Post {
    pub id: i32,
    pub author: String,
    pub author_name: Option<String>,
    pub author_type: String,
    pub title: String,
    pub body: String,
    pub category: Option<String>,
    pub views: i32,
    pub votes: i32,
    pub pinned: bool,
    pub reply_count: i32,
    pub created_at: String,
}

#[derive(Deserialize, Serialize)]
pub struct PostList {
    pub posts: Vec<Post>,
    pub total: i64,
}

#[derive(Deserialize, Serialize)]
pub struct Reply {
    pub id: i32,
    pub post_id: i32,
    pub author: String,
    pub author_name: Option<String>,
    pub author_type: String,
    pub body: String,
    pub votes: i32,
    pub created_at: String,
    pub parent_id: Option<i32>,
}

#[derive(Deserialize, Serialize)]
pub struct ReplyList {
    pub replies: Vec<Reply>,
    pub total: i64,
}

#[derive(Deserialize, Serialize)]
pub struct Me {
    pub address: String,
    pub username: Option<String>,
    pub user_type: String,
    pub is_admin: bool,
}

// ── Helper: health check, suppresses auth ──────────────────────────────

pub fn ping(base: &str) -> Result<Health> {
    let api = Api::new(base, None);
    let h = api.get_json::<Health>("/health")?;
    if h.status != "ok" {
        log_warn!("server reports status: {}", h.status);
    }
    Ok(h)
}
