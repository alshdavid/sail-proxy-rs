mod config;
mod handler_context;
mod handlers;
mod sail;
mod self_upgrade;

use std::sync::Arc;

use uhttp::Router;

use crate::handler_context::HandlerContext;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  if std::env::args()
    .collect::<Vec<String>>()
    .contains(&"--update".to_string())
  {
    self_upgrade::do_update().await?;
    return Ok(());
  } else {
    if let Some(new_version) = self_upgrade::check_for_update(&self_upgrade::UpgradeOptions {
      target_repo: self_upgrade::TARGET_REPO,
      current_version: self_upgrade::CURRENT_VERSION,
    })
    .await?
    {
      println!(
        "⚡ Update available: {} -> {}  (run `sail-proxy-rs --update`)",
        self_upgrade::CURRENT_VERSION,
        new_version
      );
    };
  }

  let config = Arc::new(config::Config::parse()?);

  let ctx = HandlerContext {
    config: config.clone(),
    client: reqwest::Client::new(),
  };

  let mut app = Router::<HandlerContext>::new(ctx).with(uhttp::middleware::logger_default);

  app.any("/health", handlers::health_any);
  app.post("/v1/chat/completions", handlers::v1_chat_completions_post);

  println!("Serving on {}", config.address);
  uhttp::Server::builder()
    .handler(app.handler())
    .listen(&config.address)
    .await?;

  Ok(())
}
