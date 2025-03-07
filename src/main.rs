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

    let anthropic_api_key =
        std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY is not set");
    let model = std::env::var("CLAUDE_MODEL").unwrap_or("claude-3-5-haiku-latest".to_string());

    let default_system_prompt = "You are a helpful assistant.".to_string();
    let system_prompt = std::env::var("CLAUDE_SYSTEM_PROMPT").unwrap_or(default_system_prompt);

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

    let anthropic_client =
        AnthropicClient::new::<MessageError>(anthropic_api_key, "2023-06-01").unwrap();

    let mut messages = Vec::new();

    print!("{} {}", ">>>".bright_blue(), "[Assistant]:".green().bold());
    println!("    {}\n", "How can I help you today?".bold());
    println!("    {}\n", "(If you want to input multiple lines, input the line and press Enter, and if you want to end, input \"///\" and press Enter.)".dimmed());

    loop {
        print!("{} ", ">>>".bright_blue());
        print!("{} ", "[User]:".green().bold());
        std::io::stdout().flush().unwrap();

        // enable multiple lines input
        let mut user_input = String::new();
        let mut current_line = String::new();

        loop {
            current_line.clear();
            std::io::stdin()
                .read_line(&mut current_line)
                .expect("Failed to read line");

            let trimmed = current_line.trim();

            // end input signal (///)
            if trimmed == "///" {
                break;
            }

            // end input signal (exit, quit, empty)
            if user_input.is_empty()
                && (trimmed.eq_ignore_ascii_case("exit")
                    || trimmed.eq_ignore_ascii_case("quit")
                    || trimmed.is_empty())
            {
                println!("{} {}", ">>>".bright_blue(), "[Assistant]:".green().bold());
                println!("    {}", "Goodbye! Have a great day!".bright_white());
                return Ok(());
            }

            // accumulate input
            user_input.push_str(trimmed);
            user_input.push('\n');

            // next line input prompt (add indent)
            if user_input.len() > 0 {
                print!("    ");
                std::io::stdout().flush().unwrap();
            }
        }

        let user_input = user_input.trim().to_string();

        if user_input.is_empty() {
            continue;
        }

        messages.push(Message::new_text(Role::User, user_input));

        let body = CreateMessageParams::new(RequiredMessageParams {
            model: model.clone(),
            messages: messages.clone(),
            max_tokens: 1024,
        })
        .with_stream(true)
        .with_system(persona.clone());

        println!(
            "\n{} {}",
            ">>>".bright_blue(),
            "[Assistant]:".green().bold()
        );

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
                println!("\n"); // 応答後に改行
            }
            Err(e) => {
                error!("API error: {}", e);
                println!("{} {}", ">>>".red(), "[Error]:".red().bold());
                println!("    {}", e.to_string().red());
                continue;
            }
        }

        messages.push(Message::new_text(Role::Assistant, assistant_response));
    }
}
