# simple-claude-cli

A simple command-line interface for interacting with Claude AI models using the Anthropic API.

## Overview

simple-claude-cli allows you to easily interact with Claude AI directly from your terminal. It uses the official Anthropic AI SDK to connect to Claude's API and stream responses in real-time.

## Features

- Real-time streaming of Claude's responses
- Colored terminal output for better readability
- Simple, intuitive interface
- Uses Claude 3.5 Haiku by default
- Customizable system prompts and predefined personas
- **Conversation history** - maintains context throughout your session
- **Multi-line input** - easily input code blocks or longer prompts
- **Configurable max tokens** - control response length

## Prerequisites

- Rust and Cargo installed
- An Anthropic API key (get one from [Anthropic's website](https://www.anthropic.com/))

## Installation

### From crates.io

Install directly from crates.io:

```bash
cargo install simple-claude-cli
```

This will install the `claude` command in your PATH.

### From Source

Clone the repository and build the project:

```bash
git clone https://github.com/katsuhirohonda/simple-claude-cli.git
cd simple-claude-cli
cargo build --release
```

The compiled binary will be available at `target/release/claude`.

## Usage

1. Set your Anthropic API key as an environment variable:

```bash
export ANTHROPIC_API_KEY=your_api_key_here
```

2. Run the application:

```bash
claude
```

3. Enter your questions or prompts. For multi-line input:
   - Type each line followed by Enter
   - Add the termination marker `///` on a new line to submit your full message
   - For code blocks or longer text, this allows proper formatting

4. Type `exit`, `quit`, or press Enter on an empty line to end the conversation.

## Configuration

You can customize Claude's behavior using environment variables:

### Change the Claude model

```bash
export CLAUDE_MODEL="claude-3-7-sonnet-20250219"
```

By default, the application uses `claude-3-5-haiku-latest`.

### Set maximum response length

Control the maximum length of Claude's responses:

```bash
export CLAUDE_MAX_TOKENS=4000
```

By default, responses are limited to 1024 tokens.

### Custom system prompts

You can set a custom system prompt:

```bash
export CLAUDE_SYSTEM_PROMPT="You are a cybersecurity expert focused on threat analysis."
```

### Predefined personas

Choose from several predefined personas:

```bash
export CLAUDE_PERSONA="engineer"  # Software engineering expert
```

Available personas:
- `engineer` - Software engineering expert
- `writer` - Creative writer with excellent language skills
- `scientist` - Scientist with expertise in various fields
- `teacher` - Patient teacher who explains concepts clearly
- `chef` - Professional chef with culinary knowledge
- `therapist` - Compassionate therapist who listens carefully

You can also set a custom persona directly:

```bash
export CLAUDE_PERSONA="You are a financial advisor specializing in retirement planning."
```

### Priority between CLAUDE_PERSONA and CLAUDE_SYSTEM_PROMPT

When configuring Claude's behavior:

- If only `CLAUDE_SYSTEM_PROMPT` is set, it will be used as the system prompt.
- If only `CLAUDE_PERSONA` is set, it will be used as the system prompt.
- If both `CLAUDE_PERSONA` and `CLAUDE_SYSTEM_PROMPT` are set, `CLAUDE_PERSONA` takes precedence and `CLAUDE_SYSTEM_PROMPT` is ignored.
- If neither is set, Claude will use the default "You are a helpful assistant." prompt.

For example, if you set:
```bash
export CLAUDE_SYSTEM_PROMPT="You are a cybersecurity expert."
export CLAUDE_PERSONA="engineer"
```
Claude will use "You are an excellent software engineer." as the system prompt, ignoring the cybersecurity expert setting.

## Dependencies

- `anthropic-ai-sdk`: Official Anthropic AI SDK for Rust
- `tokio`: Asynchronous runtime for Rust
- `tracing`: Logging and diagnostics
- `colored`: Terminal text coloring

## License

[MIT License](LICENSE)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
