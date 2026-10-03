# Providers, Models & Thinking

Pi is a multi-provider agent core. Depending on your Pi installation, its model
registry can use providers such as Anthropic, OpenAI, Google, Bedrock, Azure,
Groq, xAI and OpenRouter.

- `/model` opens the native Pager picker over Pi's available model catalog.
- `/effort` changes the thinking level supported by the selected model.
- Startup flags include `--provider`, `--model`, `--models` and `--thinking`.
- Default-on `/login` and `/logout` delegate to Pi's provider authentication
  through native QuestionView dialogs, independently of Remote TUI.
- `~/.pi/agent/models.json` can describe OpenAI-compatible local endpoints such
  as Ollama, LM Studio or vLLM.
- An extension can call `registerProvider` to add OAuth, dynamic model discovery
  or a completely custom streaming API.

Pager renders the selector; Pi still owns credentials, provider behavior and
model requests.
