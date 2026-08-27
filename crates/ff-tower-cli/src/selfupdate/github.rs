//! GitHub Releases API client for self-update.

use serde::Deserialize;

use super::failed;
use crate::error::CliError;

#[derive(Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(180)))
        .http_status_as_error(false)
        .build()
        .into()
}

pub fn get(agent: &ureq::Agent, url: &str) -> Result<ureq::http::Response<ureq::Body>, CliError> {
    let mut req = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "ff-tower");

    if let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.is_empty()
    {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    req.call()
        .map_err(|err| failed(format!("cannot reach GitHub: {err}")))
}

pub fn fetch_latest(agent: &ureq::Agent, api_base: &str) -> Result<Release, CliError> {
    let url = format!("{api_base}/repos/tyler-johnson/tower/releases/latest");
    let mut resp = get(agent, &url)?;

    let status = resp.status().as_u16();
    if status == 403
        && let Some(remaining) = resp
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
        && remaining == "0"
    {
        return Err(failed(
            "GitHub API rate limit hit — try again later, or set GITHUB_TOKEN",
        ));
    }

    if !(200..300).contains(&status) {
        return Err(failed(format!("GitHub API error: HTTP {status}")));
    }

    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|err| failed(format!("cannot read GitHub API response: {err}")))?;

    serde_json::from_str(&body).map_err(|_| failed("unexpected GitHub API response"))
}

pub fn fetch_text(agent: &ureq::Agent, url: &str) -> Result<String, CliError> {
    let mut resp = get(agent, url)?;

    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(failed(format!("download failed: HTTP {status}")));
    }

    resp.body_mut()
        .read_to_string()
        .map_err(|err| failed(format!("cannot read response body: {err}")))
}
