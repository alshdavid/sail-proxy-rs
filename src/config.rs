use std::env;

use anyhow::Context;

#[derive(Debug)]
pub struct Config {
  pub sail_api_key: String,
  pub sail_api_base: String,
}

impl Config {
  pub fn parse() -> anyhow::Result<Self> {
    let sail_api_key = env::var("SAIL_API_KEY").context("Missing SAIL_API_KEY")?;
    let sail_api_base =
      env::var("SAIL_API_BASE").unwrap_or_else(|_| "https://api.sailresearch.com/v1".to_string());

    Ok(Self {
      sail_api_key,
      sail_api_base,
    })
  }
}
