use anthropic_ai_sdk::clients::AnthropicClient;
use anthropic_ai_sdk::types::message::{
    CreateMessageParams, Message, MessageClient, MessageError, RequiredMessageParams, Role, Tool,
};
use colored::*;
use futures_util::StreamExt;
use tracing::{error, info};

///
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

    println!("{} {}", ">>>".bright_blue(), "[Assistant]:".green().bold());
    println!("    {}", "How can I help you today? If your task includes !!, it means you need to search similar tasks from the past.".bright_white());

    let mut task_description = String::new();
    std::io::stdin()
        .read_line(&mut task_description)
        .expect("Failed to read line");
    let task_description = task_description.trim().to_string();

    if task_description.is_empty() {
        println!("{} {}", ">>>".red(), "[Error]:".red().bold());
        println!(
            "    {}",
            "I need a task description to get started. Please try again.".bright_white()
        );
        std::process::exit(1);
    }

    let anthropic_api_key =
        std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY is not set");
    let anthropic_client =
        AnthropicClient::new::<MessageError>(anthropic_api_key, "2023-06-01").unwrap();

    let meta_prompt = format!(
        "You are an excellent software engineer.\n\
            There are four available reasoning methods:\n\
            1. Chain-of-Thought (CoT): Generate answers by thinking step by step.\n\
            2. Automatic Chain-of-Thought (Auto-CoT): The model explores effective reasoning paths on its own.\n\
            3. Self-Consistency: Generate multiple reasoning paths and choose the most consistent answer by majority vote.\n\
            4. Tree-of-Thought (ToT): Consider multiple approaches in parallel and derive the optimal solution.\n\n\
            First, determine which reasoning method is most appropriate and briefly explain why.\n\
            If there are past task execution procedures that are similar to the current problem, please analyze them before starting the task.\n\
            Then, considering both your chosen reasoning method and past task execution procedures, address the problem comprehensively.",
    );

    let tool_notes = "Important notes regarding tool usage:\n\
        1. Avoid using the same tool consecutively.\n\
        2. Prioritize tools suggested by the user.\n\
        3. Before using each tool, evaluate whether it is truly necessary.";

    let system_message = format!("{}\n{}", meta_prompt, tool_notes);

    let initial_prompt = &format!(
        "{:?} Please consider and execute the next action.",
        task_description
    );

    let body = CreateMessageParams::new(RequiredMessageParams {
        model: "claude-3-5-haiku-20241022".to_string(),
        messages: vec![Message::new_text(Role::User, initial_prompt)],
        max_tokens: 1024,
    })
    .with_stream(true)
    .with_system(system_message);

    match anthropic_client.create_message_streaming(&body).await {
        Ok(mut stream) => {
            while let Some(result) = stream.next().await {
                match result {
                    Ok(event) => info!("Received event: {:?}", event),
                    Err(e) => error!("Stream error: {}", e),
                }
            }
        }
        Err(e) => {
            error!("Error: {}", e);
        }
    }

    let response = match anthropic_client.create_message(Some(&body)).await {
        Ok(response) => {
            //body.messages.push(Message::new_text(
            //    Role::Assistant,
            //    format!("{:?}", response.content),
            //));
            response
        }
        Err(e) => {
            println!(
                "{} {}: {}",
                ">>>".red(),
                "[Error]".red().bold(),
                e.to_string()
            );
            error!("Error: {:?}", e);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ));
        }
    };

    Ok(())
}
