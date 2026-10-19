mod handler_context;
mod handlers;
mod sail;

use uhttp::Router;

use crate::handler_context::HandlerContext;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let mut app =
    Router::<HandlerContext>::new(HandlerContext {}).with(uhttp::middleware::logger_default);

  app.any("/health", handlers::health_any);
  app.post("/v1/chat/completions", handlers::v1_chat_completions_post);

  uhttp::Server::builder()
    .handler(app.handler())
    .listen("0.0.0.0:9090")
    .await?;

  Ok(())
}
