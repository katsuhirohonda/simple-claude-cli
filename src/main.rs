use anthropic_ai_sdk::clients::AnthropicClient;
use anthropic_ai_sdk::types::message::StreamEvent;
use anthropic_ai_sdk::types::message::{
    ContentBlockDelta, CreateMessageParams, Message, MessageClient, MessageError,
    RequiredMessageParams, Role,
};
use colored::*;
use futures_util::StreamExt;
use std::io::Write;
use tracing::error;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    // Initialize logger
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_file(false)
        .with_level(true)
        .try_init()
        .expect("Failed to initialize logger");

    // Get environment variables
    let anthropic_api_key =
        std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY is not set");
    let model = std::env::var("CLAUDE_MODEL").unwrap_or("claude-3-5-haiku-latest".to_string());

    // Get max tokens from environment or use default
    let max_tokens = std::env::var("CLAUDE_MAX_TOKENS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1024);

    // Set up system prompt
    let default_system_prompt = "You are a helpful assistant.".to_string();
    let system_prompt = std::env::var("CLAUDE_SYSTEM_PROMPT").unwrap_or(default_system_prompt);

    // Configure persona based on environment variable
    let persona = match std::env::var("CLAUDE_PERSONA").ok().as_deref() {
        Some("engineer") => "You are an excellent software engineer.".to_string(),
        Some("writer") => "You are a creative writer with excellent language skills.".to_string(),
        Some("scientist") => {
            "You are a scientist with expertise in various scientific fields.".to_string()
        }
        Some("teacher") => "You are a patient teacher who explains concepts clearly.".to_string(),
        Some("chef") => {
            "You are a professional chef with extensive culinary knowledge.".to_string()
        }
        Some("therapist") => "You are a compassionate therapist who listens carefully.".to_string(),
        Some(custom) => custom.to_string(),
        None => system_prompt,
    };

    // Initialize Anthropic client
    let anthropic_client =
        AnthropicClient::new::<MessageError>(anthropic_api_key, "2023-06-01").unwrap();

    // Initialize message history
    let mut messages = Vec::new();

    // Display welcome message and instructions
    print!("{} {}", ">>>".bright_blue(), "[Assistant]:".green().bold());
    println!("    {}\n", "How can I help you today?".bold());
    println!("    {}", "(Instructions:)".dimmed());
    println!("    {}", "1. Type your message and press Enter".dimmed());
    println!(
        "    {}",
        "2. For multiple lines, keep typing and pressing Enter".dimmed()
    );
    println!(
        "    {}",
        "3. Type \"///\" and press Enter when done with your message".dimmed()
    );
    println!(
        "    {}\n",
        "4. Type `exit`, `quit`, or press Enter on an empty line to end the conversation".dimmed()
    );
    // Main conversation loop
    loop {
        print!("{} ", ">>>".bright_blue());
        print!("{} ", "[User]:".green().bold());
        std::io::stdout().flush().unwrap();

        // Enable multiple lines input
        let mut user_input = String::new();
        let mut current_line = String::new();

        loop {
            current_line.clear();
            std::io::stdin()
                .read_line(&mut current_line)
                .expect("Failed to read line");

            let trimmed = current_line.trim();

            // End input signal (///)
            if trimmed == "///" {
                break;
            }

            // End conversation commands (exit, quit, empty)
            if user_input.is_empty()
                && (trimmed.eq_ignore_ascii_case("exit")
                    || trimmed.eq_ignore_ascii_case("quit")
                    || trimmed.is_empty())
            {
                println!("{} {}", ">>>".bright_blue(), "[Assistant]:".green().bold());
                println!("    {}", "Goodbye! Have a great day!".bright_white());
                return Ok(());
            }

            // Accumulate input
            user_input.push_str(trimmed);
            user_input.push('\n');

            // Next line input prompt (add indent)
            if user_input.len() > 0 {
                print!("    ");
                std::io::stdout().flush().unwrap();
            }
        }

        let user_input = user_input.trim().to_string();

        if user_input.is_empty() {
            continue;
        }

        // Add user message to history
        messages.push(Message::new_text(Role::User, user_input));

        // Create API request
        let body = CreateMessageParams::new(RequiredMessageParams {
            model: model.clone(),
            messages: messages.clone(),
            max_tokens: max_tokens,
        })
        .with_stream(true)
        .with_system(persona.clone());

        // Display assistant response prompt
        println!(
            "\n{} {}",
            ">>>".bright_blue(),
            "[Assistant]:".green().bold()
        );

        // Process streaming response
        let mut assistant_response = String::new();
        match anthropic_client.create_message_streaming(&body).await {
            Ok(mut stream) => {
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(event) => {
                            if let StreamEvent::ContentBlockDelta { index: _, delta } = event {
                                if let ContentBlockDelta::TextDelta { text } = delta {
                                    print!("{}", text);
                                    std::io::stdout().flush().unwrap();
                                    assistant_response.push_str(&text);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Stream error: {}", e);
                            println!("\n{} {}", ">>>".red(), "[Error]:".red().bold());
                            println!("    {}", e.to_string().red());
                        }
                    }
                }
                println!("\n"); // Add newline after response
            }
            Err(e) => {
                error!("API error: {}", e);
                println!("{} {}", ">>>".red(), "[Error]:".red().bold());
                println!("    {}", e.to_string().red());
                continue;
            }
        }

        // Add assistant response to history
        messages.push(Message::new_text(Role::Assistant, assistant_response));
    }
}
