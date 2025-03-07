# simple-claude-cli

A simple command-line interface for interacting with Claude AI models using the Anthropic API.

## Overview

claude-cli allows you to easily interact with Claude AI directly from your terminal. It uses the official Anthropic AI SDK to connect to Claude's API and stream responses in real-time.

## Features

- Real-time streaming of Claude's responses
- Colored terminal output for better readability
- Simple, intuitive interface
- Uses Claude 3.5 Haiku by default

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
cd claude-cli
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

3. Enter your question or prompt when prompted.

## Configuration

The application uses Claude 3.5 Haiku by default. To change the model or other parameters, you can modify the `src/main.rs` file.

## Dependencies

- `anthropic-ai-sdk`: Official Anthropic AI SDK for Rust
- `tokio`: Asynchronous runtime for Rust
- `tracing`: Logging and diagnostics
- `colored`: Terminal text coloring

## License

[MIT License](LICENSE)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
