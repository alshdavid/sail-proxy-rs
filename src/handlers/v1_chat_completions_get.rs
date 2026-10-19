use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde_json::json;
use tokio::io::AsyncWriteExt;
use uhttp::Request;
use uhttp::Response;

use crate::handler_context::HandlerContext;

pub async fn v1_chat_completions_post(
  mut req: Request,
  res: Response,
  ctx: HandlerContext,
) -> uhttp::HandlerResult {
  let body = uhttp::body::json::<serde_json::Value>(&mut req.body).await?;

  if body
    .get("stream")
    .and_then(|v| v.as_bool())
    .unwrap_or(false)
  {
    handle_stream(req, res, ctx, body).await
  } else {
    handle_blocking(req, res, ctx, body).await
  }
}

async fn handle_blocking(
  _req: Request,
  res: Response,
  _ctx: HandlerContext,
  body: serde_json::Value,
) -> uhttp::HandlerResult {
  let created = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_secs();
  let model = body["model"].as_str().unwrap_or("mock-model");

  let response = json!({
    "id": format!("chatcmpl-{created}"),
    "object": "chat.completion",
    "created": created,
    "model": model,
    "choices": [{
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "This is a mock response."
      },
      "finish_reason": "stop"
    }],
    "usage": {
      "prompt_tokens": 0,
      "completion_tokens": 0,
      "total_tokens": 0
    }
  });

  res
    .header(uhttp::headers::CONTENT_TYPE, "application/json")
    .json(response)
}

async fn handle_stream(
  _req: Request,
  res: Response,
  _ctx: HandlerContext,
  body: serde_json::Value,
) -> uhttp::HandlerResult {
  let created = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_secs();
  let id = format!("chatcmpl-{created}");
  let model = body["model"].as_str().unwrap_or("mock-model").to_string();
  let include_usage = body["stream_options"]["include_usage"].as_bool() == Some(true);
  let (response_body, mut stream) = uhttp::ResponseBody::chunked(64);

  tokio::spawn(async move {
    let chunk = |delta: serde_json::Value, finish_reason: Option<&str>| {
      json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{ "index": 0, "delta": delta, "finish_reason": finish_reason }]
      })
    };

    // Opening chunk, sent immediately
    let first = chunk(json!({ "role": "assistant", "content": "" }), None);
    if stream
      .write_all(format!("data: {first}\n\n").as_bytes())
      .await
      .is_err()
    {
      return;
    }

    // Long-running work with heartbeats
    let job = tokio::time::sleep(Duration::from_secs(10)); // replace with real work
    tokio::pin!(job);

    let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
    heartbeat.tick().await; // first tick is immediate

    loop {
      tokio::select! {
        _ = &mut job => break,
        _ = heartbeat.tick() => {
          if stream.write_all(b": keep-alive\n\n").await.is_err() {
            return; // client gone; returning drops `job`, cancelling it
          }
        }
      }
    }

    // Final frames (role already sent, so only content here)
    let mut frames = vec![
      chunk(json!({ "content": "This is a mock response." }), None),
      chunk(json!({}), Some("stop")),
    ];

    if include_usage {
      frames.push(json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [],
        "usage": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 }
      }));
    }

    for frame in frames {
      if stream
        .write_all(format!("data: {frame}\n\n").as_bytes())
        .await
        .is_err()
      {
        return;
      }
    }

    let _ = stream.write_all(b"data: [DONE]\n\n").await;
    let _ = stream.shutdown().await;
  });

  res
    .header(uhttp::headers::CONTENT_TYPE, "text/event-stream")
    .header(uhttp::headers::CACHE_CONTROL, "no-cache")
    .header("x-accel-buffering", "no")
    .body(response_body)
}
