use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use uhttp::Request;
use uhttp::Response;

use crate::handler_context::HandlerContext;
use crate::sail;

const POLL_INTERVAL: Duration = Duration::from_secs(2);

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
  ctx: HandlerContext,
  body: serde_json::Value,
) -> uhttp::HandlerResult {
  let request = match to_sail_request(&body) {
    Ok(request) => request,
    Err(error) => return error_response(res, 400, "invalid_request_error", &error),
  };

  let idempotency_key = uuid::Uuid::new_v4().to_string();
  let pending = match sail::create_background_response(
    &ctx.client,
    &ctx.config.sail_api_base,
    &ctx.config.sail_api_key,
    &idempotency_key,
    &request,
  )
  .await
  {
    Ok(pending) => pending,
    Err(error) => return error_response(res, 502, "server_error", &error),
  };

  let created = pending.created_at;
  let id = pending.id.clone();
  let model = pending.model.clone();
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

    let job = wait_for_completion(&ctx, pending);
    tokio::pin!(job);

    let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
    heartbeat.tick().await; // first tick is immediate

    let result = loop {
      tokio::select! {
        result = &mut job => break result,
        _ = heartbeat.tick() => {
          if stream.write_all(b": keep-alive\n\n").await.is_err() {
            return; // client gone; returning drops `job`, cancelling it
          }
        }
      }
    };

    let completion = match result {
      Ok(response) => to_completion(&response).map(|completion| (response, completion)),
      Err(error) => Err(json!({ "message": format!("{error:#}"), "type": "server_error" })),
    };

    let mut frames = Vec::new();
    match completion {
      Ok((response, completion)) => {
        frames.extend(
          completion
            .deltas
            .into_iter()
            .map(|delta| chunk(delta, None)),
        );
        frames.push(chunk(json!({}), Some(completion.finish_reason)));

        if include_usage {
          frames.push(json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model,
            "choices": [],
            "usage": to_chat_usage(&response.usage.unwrap_or_default()),
          }));
        }
      }
      Err(error) => frames.push(json!({ "error": error })),
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

fn error_response(
  res: Response,
  status: u16,
  kind: &str,
  error: &anyhow::Error,
) -> uhttp::HandlerResult {
  res
    .status(status)
    .json(json!({ "error": { "message": format!("{error:#}"), "type": kind } }))
}

async fn wait_for_completion(
  ctx: &HandlerContext,
  mut response: sail::SailResponse,
) -> anyhow::Result<sail::SailResponse> {
  while !response.status.is_terminal() {
    tokio::time::sleep(POLL_INTERVAL).await;
    if let Some(next) = sail::get_response(
      &ctx.client,
      &ctx.config.sail_api_base,
      &ctx.config.sail_api_key,
      &response.id,
    )
    .await?
    {
      response = next;
    }
  }
  Ok(response)
}

fn to_sail_request(body: &Value) -> anyhow::Result<sail::CreateResponseRequest> {
  let mut input = Vec::new();
  for message in body["messages"].as_array().context("missing messages")? {
    push_input_items(&mut input, message)?;
  }

  let tools = body["tools"]
    .as_array()
    .into_iter()
    .flatten()
    .map(to_sail_tool)
    .collect::<anyhow::Result<Vec<_>>>()?;

  let tool_choice = match &body["tool_choice"] {
    Value::Null => None,
    Value::String(_) => Some(sail::ToolChoice::Mode(
      serde_json::from_value(body["tool_choice"].clone()).context("invalid tool_choice")?,
    )),
    choice => Some(sail::ToolChoice::Function(sail::FunctionToolChoice {
      name: required_str(&choice["function"]["name"], "tool_choice.function.name")?,
    })),
  };

  let text = match body["response_format"]["type"].as_str() {
    None => None,
    Some("text") => Some(sail::TextConfig {
      format: sail::TextFormat::Text,
    }),
    Some("json_schema") => {
      let schema = &body["response_format"]["json_schema"];
      Some(sail::TextConfig {
        format: sail::TextFormat::JsonSchema {
          name: required_str(&schema["name"], "response_format.json_schema.name")?,
          description: schema["description"].as_str().map(str::to_string),
          schema: non_null(&schema["schema"]),
          strict: schema["strict"].as_bool(),
        },
      })
    }
    Some(other) => anyhow::bail!("unsupported response_format: {other}"),
  };

  let reasoning = non_null(&body["reasoning_effort"])
    .map(serde_json::from_value)
    .transpose()
    .context("invalid reasoning_effort")?
    .map(|effort| sail::ReasoningConfig {
      effort: Some(effort),
      generate_summary: Some(sail::ReasoningSummary::Auto),
    });

  Ok(sail::CreateResponseRequest {
    model: required_str(&body["model"], "model")?,
    input,
    instructions: None,
    tools,
    tool_choice,
    parallel_tool_calls: body["parallel_tool_calls"].as_bool(),
    max_output_tokens: body["max_completion_tokens"]
      .as_u64()
      .or(body["max_tokens"].as_u64()),
    temperature: body["temperature"].as_f64(),
    top_p: body["top_p"].as_f64(),
    text,
    reasoning,
    user: body["user"].as_str().map(str::to_string),
  })
}

fn push_input_items(
  input: &mut Vec<sail::InputItem>,
  message: &Value,
) -> anyhow::Result<()> {
  let content = &message["content"];
  match message["role"].as_str() {
    Some("system" | "developer") => input.push(text_message(sail::Role::System, content)),
    Some("user") => input.push(sail::InputItem::Message(sail::InputMessage {
      role: sail::Role::User,
      content: to_user_content(content)?,
    })),
    Some("assistant") => {
      if !content_text(content).is_empty() {
        input.push(text_message(sail::Role::Assistant, content));
      }
      for call in message["tool_calls"].as_array().into_iter().flatten() {
        input.push(sail::InputItem::FunctionCall(sail::FunctionCall {
          call_id: required_str(&call["id"], "tool_calls[].id")?,
          name: required_str(&call["function"]["name"], "tool_calls[].function.name")?,
          arguments: call["function"]["arguments"]
            .as_str()
            .unwrap_or("{}")
            .to_string(),
        }));
      }
    }
    Some("tool") => input.push(sail::InputItem::FunctionCallOutput(
      sail::FunctionCallOutput {
        call_id: required_str(&message["tool_call_id"], "tool_call_id")?,
        output: content_text(content),
      },
    )),
    role => anyhow::bail!("unsupported message role: {role:?}"),
  }
  Ok(())
}

fn text_message(
  role: sail::Role,
  content: &Value,
) -> sail::InputItem {
  sail::InputItem::Message(sail::InputMessage {
    role,
    content: sail::MessageContent::Text(content_text(content)),
  })
}

fn content_text(content: &Value) -> String {
  match content {
    Value::String(text) => text.clone(),
    Value::Array(parts) => parts
      .iter()
      .filter_map(|part| part["text"].as_str())
      .collect(),
    _ => String::new(),
  }
}

fn to_user_content(content: &Value) -> anyhow::Result<sail::MessageContent> {
  let Value::Array(parts) = content else {
    return Ok(sail::MessageContent::Text(content_text(content)));
  };

  let parts = parts
    .iter()
    .map(|part| match part["type"].as_str() {
      Some("text") => Ok(sail::InputContent::Text {
        text: required_str(&part["text"], "content[].text")?,
      }),
      Some("image_url") => Ok(sail::InputContent::Image {
        image_url: required_str(&part["image_url"]["url"], "content[].image_url.url")?,
        detail: non_null(&part["image_url"]["detail"])
          .map(serde_json::from_value)
          .transpose()
          .context("invalid content[].image_url.detail")?,
      }),
      Some("file") => Ok(sail::InputContent::File {
        file_data: required_str(&part["file"]["file_data"], "content[].file.file_data")?,
        filename: part["file"]["filename"].as_str().map(str::to_string),
      }),
      kind => anyhow::bail!("unsupported content part: {kind:?}"),
    })
    .collect::<anyhow::Result<Vec<_>>>()?;

  Ok(sail::MessageContent::Parts(parts))
}

fn to_sail_tool(tool: &Value) -> anyhow::Result<sail::FunctionTool> {
  if tool["type"] != "function" {
    anyhow::bail!("unsupported tool type: {}", tool["type"]);
  }
  let function = &tool["function"];
  Ok(sail::FunctionTool {
    name: required_str(&function["name"], "tools[].function.name")?,
    description: function["description"].as_str().map(str::to_string),
    parameters: non_null(&function["parameters"]),
    strict: function["strict"].as_bool(),
  })
}

fn required_str(
  value: &Value,
  field: &str,
) -> anyhow::Result<String> {
  value
    .as_str()
    .map(str::to_string)
    .with_context(|| format!("missing {field}"))
}

fn non_null(value: &Value) -> Option<Value> {
  (!value.is_null()).then(|| value.clone())
}

struct Completion {
  deltas: Vec<Value>,
  finish_reason: &'static str,
}

fn to_completion(response: &sail::SailResponse) -> Result<Completion, Value> {
  if matches!(
    response.status,
    sail::ResponseStatus::Failed | sail::ResponseStatus::Cancelled
  ) {
    let error = response.error.as_ref();
    return Err(json!({
      "message": error
        .and_then(|error| error.message.clone())
        .unwrap_or_else(|| format!("Sail response {:?}", response.status)),
      "type": error
        .and_then(|error| error.kind.clone())
        .unwrap_or_else(|| "server_error".to_string()),
      "param": error.and_then(|error| error.param.clone()),
      "code": error.and_then(|error| error.code.clone()),
    }));
  }

  let mut reasoning = String::new();
  let mut content = String::new();
  let mut refusal = String::new();
  let mut tool_calls = Vec::new();

  for item in response.output.iter().flatten() {
    match item {
      sail::OutputItem::Message { content: parts } => {
        for part in parts {
          match part {
            sail::OutputContent::OutputText { text } => content.push_str(text),
            sail::OutputContent::Refusal { refusal: text } => refusal.push_str(text),
            sail::OutputContent::Unknown => {}
          }
        }
      }
      sail::OutputItem::FunctionCall {
        call_id,
        name,
        arguments,
      } => tool_calls.push(json!({
        "index": tool_calls.len(),
        "id": call_id,
        "type": "function",
        "function": { "name": name, "arguments": arguments },
      })),
      sail::OutputItem::Reasoning { summary, content } => {
        for part in summary.iter().chain(content.iter()).flatten() {
          match part {
            sail::ReasoningContent::SummaryText { text }
            | sail::ReasoningContent::ReasoningText { text } => reasoning.push_str(text),
            sail::ReasoningContent::Unknown => {}
          }
        }
      }
      sail::OutputItem::Unknown => {}
    }
  }

  let finish_reason = if response.status == sail::ResponseStatus::Incomplete {
    match response
      .incomplete_details
      .as_ref()
      .and_then(|details| details.reason)
    {
      Some(sail::IncompleteReason::ContentFilter) => "content_filter",
      _ => "length",
    }
  } else if tool_calls.is_empty() {
    "stop"
  } else {
    "tool_calls"
  };

  let mut deltas = Vec::new();
  if !reasoning.is_empty() {
    deltas.push(json!({ "reasoning_content": reasoning }));
  }
  if !content.is_empty() {
    deltas.push(json!({ "content": content }));
  }
  if !refusal.is_empty() {
    deltas.push(json!({ "refusal": refusal }));
  }
  if !tool_calls.is_empty() {
    deltas.push(json!({ "tool_calls": tool_calls }));
  }

  Ok(Completion {
    deltas,
    finish_reason,
  })
}

fn to_chat_usage(usage: &sail::Usage) -> Value {
  json!({
    "prompt_tokens": usage.input_tokens,
    "completion_tokens": usage.output_tokens,
    "total_tokens": usage.total_tokens,
    "prompt_tokens_details": { "cached_tokens": usage.input_tokens_details.cached_tokens },
    "completion_tokens_details": { "reasoning_tokens": usage.output_tokens_details.reasoning_tokens },
  })
}
