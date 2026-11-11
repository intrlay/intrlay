//! Probe registry OIDC authentication from the release workflow without publishing.

use std::env;
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use clap::{Args, ValueEnum};
use serde_json::{Value, json};

#[derive(Args)]
pub(crate) struct Options {
    #[arg(value_enum)]
    registry: Registry,
}

#[derive(Clone, Copy, ValueEnum)]
enum Registry {
    Crates,
    Npm,
    Pypi,
    Nuget,
    Pubdev,
}

fn mask(secret: &str) {
    let escaped = secret
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A");
    println!("::add-mask::{escaped}");
}

fn response_json(mut response: ureq::http::Response<ureq::Body>) -> Result<Value> {
    let status = response.status();
    let body = response.body_mut().read_to_string()?;
    ensure!(
        status.is_success(),
        "registry returned HTTP {status}: {body}"
    );
    serde_json::from_str(&body).context("registry response was not JSON")
}

fn require_token<'a>(response: &'a Value, field: &str) -> Result<&'a str> {
    let token = response[field]
        .as_str()
        .filter(|value| !value.is_empty())
        .with_context(|| format!("response did not contain {field}"))?;
    mask(token);
    Ok(token)
}

pub(crate) fn run(options: &Options) -> Result<()> {
    ensure!(
        env::var("GITHUB_ACTIONS").as_deref() == Ok("true"),
        "run this check in GitHub Actions with id-token: write"
    );
    let request_url = env::var("ACTIONS_ID_TOKEN_REQUEST_URL")?;
    let request_token = env::var("ACTIONS_ID_TOKEN_REQUEST_TOKEN")?;
    mask(&request_token);

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .max_redirects(0)
        .build();
    let agent: ureq::Agent = config.into();
    let audience = match options.registry {
        Registry::Crates => "crates.io",
        Registry::Npm => "npm:registry.npmjs.org",
        Registry::Pypi => "pypi",
        Registry::Nuget => "https://www.nuget.org",
        Registry::Pubdev => "https://pub.dev",
    };

    let identity = response_json(
        agent
            .get(&request_url)
            .query("audience", audience)
            .header("Authorization", format!("Bearer {request_token}"))
            .call()?,
    )?;
    let jwt = require_token(&identity, "value")?;

    // pub.dev accepts the GitHub JWT directly. This endpoint authenticates the
    // caller, but package/version authorization happens only after an upload.
    if matches!(options.registry, Registry::Pubdev) {
        let response = response_json(
            agent
                .get("https://pub.dev/api/packages/versions/new")
                .header("Authorization", format!("Bearer {jwt}"))
                .call()?,
        )?;
        ensure!(
            response["url"].is_string() && response["fields"].is_object(),
            "pub.dev did not return an authenticated upload session"
        );
        println!("pub.dev accepted the GitHub OIDC identity; no archive was uploaded.");
        println!(
            "::warning::pub.dev package authorization is NOT verified: it also checks the tag, repository, and environment when processing an uploaded package."
        );
        return Ok(());
    }

    let (endpoint, body, bearer, token_field) = match options.registry {
        Registry::Crates => (
            "https://crates.io/api/v1/trusted_publishing/tokens",
            json!({ "jwt": jwt }),
            false,
            "token",
        ),
        Registry::Npm => (
            "https://registry.npmjs.org/-/npm/v1/oidc/token/exchange/package/%40intrlay%2Fruntime",
            json!({}),
            true,
            "token",
        ),
        Registry::Pypi => (
            "https://pypi.org/_/oidc/mint-token",
            json!({ "token": jwt }),
            false,
            "token",
        ),
        Registry::Nuget => (
            "https://www.nuget.org/api/v2/token",
            json!({ "username": env::var("NUGET_USER")?, "tokenType": "ApiKey" }),
            true,
            "apiKey",
        ),
        Registry::Pubdev => unreachable!("pub.dev uses the GitHub JWT directly"),
    };
    let mut request = agent
        .post(endpoint)
        .header("User-Agent", "intrlay-publishing-auth-check")
        .header("Content-Type", "application/json");
    if bearer {
        request = request.header("Authorization", format!("Bearer {jwt}"));
    }
    let response = response_json(request.send(serde_json::to_vec(&body)?)?)?;
    require_token(&response, token_field)?;
    println!("{audience}: OIDC exchange accepted; a registry credential was issued.");
    println!("No package upload was attempted.");
    Ok(())
}
