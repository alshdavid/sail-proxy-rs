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
  }

  let config = config::Config::parse()?;

  let ctx = HandlerContext {
    config: Arc::new(config),
    client: reqwest::Client::new(),
  };

  let mut app = Router::<HandlerContext>::new(ctx).with(uhttp::middleware::logger_default);

  app.any("/health", handlers::health_any);
  app.post("/v1/chat/completions", handlers::v1_chat_completions_post);

  uhttp::Server::builder()
    .handler(app.handler())
    .listen("0.0.0.0:9090")
    .await?;

  Ok(())
}
