#![allow(unused)]
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateResponseRequest {
  pub model: String,
  pub input: Vec<InputItem>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub instructions: Option<String>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub tools: Vec<FunctionTool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tool_choice: Option<ToolChoice>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parallel_tool_calls: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub max_output_tokens: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub temperature: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub top_p: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub text: Option<TextConfig>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reasoning: Option<ReasoningConfig>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub user: Option<String>,
}

// Messages are serialized without a `type` field because Sail's input schema
// for message items does not list one and rejects additional properties.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum InputItem {
  Message(InputMessage),
  FunctionCall(FunctionCall),
  FunctionCallOutput(FunctionCallOutput),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
  System,
  User,
  Assistant,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputMessage {
  pub role: Role,
  pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
  Text(String),
  Parts(Vec<InputContent>),
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum InputContent {
  #[serde(rename = "input_text")]
  Text { text: String },
  #[serde(rename = "input_image")]
  Image {
    image_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<ImageDetail>,
  },
  #[serde(rename = "input_file")]
  File {
    file_data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
  },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageDetail {
  Auto,
  Low,
  High,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename = "function_call")]
pub struct FunctionCall {
  pub call_id: String,
  pub name: String,
  pub arguments: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename = "function_call_output")]
pub struct FunctionCallOutput {
  pub call_id: String,
  pub output: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename = "function")]
pub struct FunctionTool {
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub description: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parameters: Option<serde_json::Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub strict: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ToolChoice {
  Mode(ToolChoiceMode),
  Function(FunctionToolChoice),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoiceMode {
  None,
  Auto,
  Required,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename = "function")]
pub struct FunctionToolChoice {
  pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextConfig {
  pub format: TextFormat,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TextFormat {
  Text,
  JsonSchema {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    strict: Option<bool>,
  },
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ReasoningConfig {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub effort: Option<ReasoningEffort>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub generate_summary: Option<ReasoningSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
  None,
  Minimal,
  Low,
  Medium,
  High,
  Xhigh,
  Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningSummary {
  Auto,
  Concise,
  Detailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
  Queued,
  InProgress,
  Completed,
  Incomplete,
  Failed,
  Cancelled,
}

impl ResponseStatus {
  pub fn is_terminal(self) -> bool {
    matches!(
      self,
      Self::Completed | Self::Incomplete | Self::Failed | Self::Cancelled
    )
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputItem {
  Message {
    #[serde(default)]
    content: Vec<OutputContent>,
  },
  FunctionCall {
    call_id: String,
    name: String,
    arguments: String,
  },
  Reasoning {
    summary: Option<Vec<ReasoningContent>>,
    content: Option<Vec<ReasoningContent>>,
  },
  #[serde(other)]
  Unknown,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputContent {
  OutputText {
    text: String,
  },
  Refusal {
    refusal: String,
  },
  #[serde(other)]
  Unknown,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReasoningContent {
  SummaryText {
    text: String,
  },
  ReasoningText {
    text: String,
  },
  #[serde(other)]
  Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseError {
  pub message: Option<String>,
  #[serde(rename = "type")]
  pub kind: Option<String>,
  pub param: Option<String>,
  // Documented as string, integer or null.
  pub code: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IncompleteDetails {
  pub reason: Option<IncompleteReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncompleteReason {
  MaxOutputTokens,
  ContentFilter,
  #[serde(other)]
  Other,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Usage {
  pub input_tokens: u64,
  #[serde(default)]
  pub input_tokens_details: UsageDetails,
  pub output_tokens: u64,
  #[serde(default)]
  pub output_tokens_details: UsageDetails,
  pub total_tokens: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UsageDetails {
  #[serde(default)]
  pub cached_tokens: u64,
  #[serde(default)]
  pub reasoning_tokens: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SailResponse {
  pub id: String,
  pub created_at: u64,
  pub status: ResponseStatus,
  pub model: String,
  pub output: Option<Vec<OutputItem>>,
  pub error: Option<ResponseError>,
  pub incomplete_details: Option<IncompleteDetails>,
  pub usage: Option<Usage>,
}

#[derive(Serialize)]
struct BackgroundRequest<'a> {
  #[serde(flatten)]
  request: &'a CreateResponseRequest,
  background: bool,
  metadata: serde_json::Value,
}

pub async fn create_background_response(
  client: &reqwest::Client,
  base_url: &str,
  api_key: &str,
  idempotency_key: &str,
  request: &CreateResponseRequest,
) -> anyhow::Result<SailResponse> {
  let res = client
    .post(format!("{base_url}/responses"))
    .bearer_auth(api_key)
    .header("Idempotency-Key", idempotency_key)
    .json(&BackgroundRequest {
      request,
      background: true,
      metadata: json!({ "completion_window": "flex" }),
    })
    .send()
    .await?;

  // Accepted background requests return 202, which is_success() covers.
  let status = res.status();
  if !status.is_success() {
    let text = res.text().await.unwrap_or_default();
    anyhow::bail!("Create failed: HTTP {status} {text}");
  }

  Ok(res.json::<SailResponse>().await?)
}

pub async fn get_response(
  client: &reqwest::Client,
  base_url: &str,
  api_key: &str,
  id: &str,
) -> anyhow::Result<Option<SailResponse>> {
  let mut url = reqwest::Url::parse(base_url)?;
  url
    .path_segments_mut()
    .map_err(|_| anyhow::anyhow!("invalid base URL: {base_url}"))?
    .pop_if_empty()
    .push("responses")
    .push(id); // percent-encodes `id`, like encodeURIComponent

  let res = client.get(url).bearer_auth(api_key).send().await?;

  // Treat rate limits and server errors as transient: signal "try again".
  let status = res.status();
  if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
    eprintln!("  poll got HTTP {}, will retry", status.as_u16());
    return Ok(None);
  }
  if !status.is_success() {
    let text = res.text().await.unwrap_or_default();
    anyhow::bail!("Retrieve failed: HTTP {} {text}", status.as_u16());
  }

  Ok(Some(res.json::<SailResponse>().await?))
}

/*
Example:

mod config;
mod handler_context;
mod handlers;
mod sail;

use uhttp::Router;

use crate::handler_context::HandlerContext;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let config = config::Config::parse()?;

  let client = reqwest::Client::new();
  let idempotency_key = uuid::Uuid::new_v4().to_string();

  let response = sail::create_background_response(
    &client,
    &config.sail_api_base,
    &config.sail_api_key,
    &idempotency_key,
    &sail::CreateResponseRequest {
      model: "deepseek-ai/DeepSeek-V4.1-Flash".to_string(),
      input: Vec::<sail::InputItem>::from_iter([sail::InputItem::Message(sail::InputMessage {
        role: sail::Role::User,
        content: sail::MessageContent::Text("Hello, how are you?".to_string()),
      })]),
      instructions: None,
      tools: vec![],
      tool_choice: None,
      parallel_tool_calls: None,
      max_output_tokens: None,
      temperature: None,
      top_p: None,
      text: None,
      reasoning: None,
      user: None,
    },
  )
  .await?;

  println!("created {} ({:?})", response.id, response.status);

  let mut last_status = response.status;
  let response = loop {
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    let Some(response) = sail::get_response(
      &client,
      &config.sail_api_base,
      &config.sail_api_key,
      &response.id,
    )
    .await?
    else {
      continue;
    };

    if response.status != last_status {
      println!("status: {:?}", response.status);
      last_status = response.status;
    }

    if matches!(
      response.status,
      sail::ResponseStatus::Completed
        | sail::ResponseStatus::Incomplete
        | sail::ResponseStatus::Failed
        | sail::ResponseStatus::Cancelled
    ) {
      break response;
    }
  };

  for item in response.output.iter().flatten() {
    match item {
      sail::OutputItem::Message { content } => {
        for part in content {
          match part {
            sail::OutputContent::OutputText { text } => println!("text: {text}"),
            sail::OutputContent::Refusal { refusal } => println!("refusal: {refusal}"),
            sail::OutputContent::Unknown => println!("unknown content part"),
          }
        }
      }
      sail::OutputItem::FunctionCall {
        call_id,
        name,
        arguments,
      } => println!("tool call {call_id}: {name}({arguments})"),
      sail::OutputItem::Reasoning { summary, content } => {
        for part in summary.iter().chain(content.iter()).flatten() {
          match part {
            sail::ReasoningContent::SummaryText { text }
            | sail::ReasoningContent::ReasoningText { text } => println!("reasoning: {text}"),
            sail::ReasoningContent::Unknown => println!("unknown reasoning part"),
          }
        }
      }
      sail::OutputItem::Unknown => println!("unknown output item"),
    }
  }

  if let Some(details) = &response.incomplete_details {
    println!("incomplete: {:?}", details.reason);
  }
  if let Some(error) = &response.error {
    println!("error: {error:?}");
  }
  if let Some(usage) = &response.usage {
    println!("usage: {usage:?}");
  }

  Ok(())
}
*/
