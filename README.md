# sail-proxy-rs

An OpenAI Chat Completions proxy for [Sail Research](https://docs.sailresearch.com) `flex` mode.

Sail's flex window only runs as a background job on the Responses API, which existing tooling can't use. This proxy accepts a normal `/v1/chat/completions` request, submits it to Sail as a flex background job, polls until it finishes, and streams the result back. Tool calls are supported.

## Run

```sh
SAIL_API_KEY=... cargo run
```

Listens on `0.0.0.0:9090`. Set `SAIL_API_BASE` to override the default `https://api.sailresearch.com/v1`.

## Use

Point any OpenAI-compatible client at `http://localhost:9090/v1`. For example, in opencode:

```json
{
  "provider": {
    "sail": {
      "npm": "@ai-sdk/openai-compatible",
      "options": { "baseURL": "http://localhost:9090/v1" },
      "models": {
        "deepseek-flash": { "id": "deepseek-ai/DeepSeek-V4.1-Flash" }
      }
    }
  }
}
```

## Limitations

- Only streaming requests (`"stream": true`) are proxied; non-streaming requests return a mock response.
- The response arrives in one go once the job finishes; keep-alives are sent while waiting.
- `response_format: json_object` and non-function tools are not supported by Sail.
