mod handlers;
mod handler_context;

use uhttp::Router;

use crate::handler_context::HandlerContext;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut app = Router::<HandlerContext>::new(HandlerContext {  });

    app.any("/health", handlers::health_any);
    app.get("/v1/chat/completions", handlers::v1_chat_completions_get);

    uhttp::Server::builder().handler(app.handler()).listen("0.0.0.0:9090").await?;

    Ok(())
}