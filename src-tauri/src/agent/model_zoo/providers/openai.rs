// OpenAI Provider Implementation for Model Zoo
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use reqwest::Client;
use std::time::Duration;
use crate::agent::model_zoo::{ModelConfig, ModelInterface};

#[derive(Debug, Clone)]
pub struct OpenAIModel {
    config: ModelConfig,
    client: Client,
    api_key: String,
}

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: MessageContent,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Multimodal(Vec<ContentPart>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Serialize, Deserialize)]
struct ImageUrl {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    index: usize,
    message: ResponseMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    role: String,
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: FunctionCall,
}

#[derive(Debug, Deserialize)]
struct FunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    index: usize,
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

impl OpenAIModel {
    pub async fn new(config: ModelConfig) -> Result<Self, String> {
        let api_key = config.api_key.clone()
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .ok_or_else(|| "OpenAI API key not found. Set OPENAI_API_KEY environment variable.".to_string())?;
        
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        
        Ok(Self {
            config,
            client,
            api_key,
        })
    }
    
    fn get_model_name(&self) -> String {
        // Extract model name from model_string (e.g., "openai/gpt-4o" -> "gpt-4o")
        let model = self.config.model_string
            .strip_prefix("openai/")
            .unwrap_or(&self.config.model_string);
        
        // Map common aliases to actual model names
        match model {
            "gpt-4o" => "gpt-4o".to_string(),
            "gpt-4o-mini" => "gpt-4o-mini".to_string(),
            "gpt-4-turbo" => "gpt-4-turbo-preview".to_string(),
            "gpt-4-vision" => "gpt-4-vision-preview".to_string(),
            "gpt-3.5-turbo" => "gpt-3.5-turbo-0125".to_string(),
            "o1-preview" => "o1-preview".to_string(),
            "o1-mini" => "o1-mini".to_string(),
            _ => model.to_string(),
        }
    }
    
    fn is_o1_model(&self) -> bool {
        let model = self.get_model_name();
        model.starts_with("o1-")
    }
    
    async fn make_request(&self, messages: Vec<OpenAIMessage>) -> Result<String, String> {
        let endpoint = self.config.endpoint.as_deref()
            .unwrap_or("https://api.openai.com/v1/chat/completions");
        
        let mut request_body = OpenAIRequest {
            model: self.get_model_name(),
            messages,
            max_tokens: if self.is_o1_model() { None } else { self.config.max_tokens },
            temperature: if self.is_o1_model() { 1.0 } else { self.config.temperature },
            stream: Some(false),
            response_format: None,
        };
        
        // Add JSON mode for compatible models
        if !self.is_o1_model() && self.get_model_name().contains("gpt-4") {
            // Can enable JSON mode if needed
        }
        
        let response = self.client
            .post(endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("OpenAI API error ({}): {}", status, error_text));
        }
        
        let openai_response: OpenAIResponse = response.json().await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        
        // Extract text from response
        let text = openai_response.choices
            .first()
            .and_then(|c| c.message.content.as_ref())
            .ok_or_else(|| "Empty response from OpenAI API".to_string())?;
        
        Ok(text.clone())
    }
}

#[async_trait]
impl ModelInterface for OpenAIModel {
    async fn generate(&self, prompt: String, images: Option<Vec<Vec<u8>>>) -> Result<String, String> {
        let content = if let Some(images) = images {
            // Create multimodal content with text and images
            let mut parts = vec![ContentPart::Text { text: prompt }];
            
            for image_data in images {
                // Convert to base64 data URL
                let base64_data = base64::encode(&image_data);
                let data_url = format!("data:image/png;base64,{}", base64_data);
                
                parts.push(ContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: data_url,
                        detail: Some("high".to_string()),
                    },
                });
            }
            
            MessageContent::Multimodal(parts)
        } else {
            MessageContent::Text(prompt)
        };
        
        let message = OpenAIMessage {
            role: "user".to_string(),
            content,
        };
        
        self.make_request(vec![message]).await
    }
    
    async fn stream_generate(&self, prompt: String) -> Result<tokio::sync::mpsc::Receiver<String>, String> {
        // Create a channel for streaming
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        
        let endpoint = self.config.endpoint.as_deref()
            .unwrap_or("https://api.openai.com/v1/chat/completions");
        
        let message = OpenAIMessage {
            role: "user".to_string(),
            content: MessageContent::Text(prompt),
        };
        
        let request_body = OpenAIRequest {
            model: self.get_model_name(),
            messages: vec![message],
            max_tokens: if self.is_o1_model() { None } else { self.config.max_tokens },
            temperature: if self.is_o1_model() { 1.0 } else { self.config.temperature },
            stream: Some(true),
            response_format: None,
        };
        
        let api_key = self.api_key.clone();
        let client = self.client.clone();
        
        // Spawn async task to handle streaming
        tokio::spawn(async move {
            let response = match client
                .post(endpoint)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    let _ = tx.send(format!("[ERROR] Request failed: {}", e)).await;
                    return;
                }
            };
            
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                let _ = tx.send(format!("[ERROR] API error ({}): {}", status, error_text)).await;
                return;
            }
            
            let mut stream = response.bytes_stream();
            use futures_util::StreamExt;
            
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        // Parse SSE format
                        let text = String::from_utf8_lossy(&bytes);
                        for line in text.lines() {
                            if line.starts_with("data: ") {
                                let data = &line[6..];
                                if data == "[DONE]" {
                                    break;
                                }
                                
                                // Parse JSON and extract text
                                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                                    if let Some(choice) = chunk.choices.first() {
                                        if let Some(content) = &choice.delta.content {
                                            if tx.send(content.clone()).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(format!("[ERROR] Stream error: {}", e)).await;
                        break;
                    }
                }
            }
        });
        
        Ok(rx)
    }
    
    fn supports_vision(&self) -> bool {
        let model = self.get_model_name();
        // GPT-4 Vision, GPT-4o, and GPT-4 Turbo support vision
        model.contains("gpt-4-vision") || 
        model.contains("gpt-4o") || 
        model.contains("gpt-4-turbo")
    }
    
    fn supports_tools(&self) -> bool {
        let model = self.get_model_name();
        // All GPT-4 and GPT-3.5 models support function calling
        // o1 models do not support function calling
        !model.starts_with("o1-") && (model.contains("gpt-4") || model.contains("gpt-3.5"))
    }
    
    fn get_context_window(&self) -> usize {
        let model = self.get_model_name();
        match model.as_str() {
            m if m.contains("gpt-4o") => 128000,
            m if m.contains("gpt-4-turbo") => 128000,
            m if m.contains("gpt-4-vision") => 128000,
            m if m.contains("gpt-4-32k") => 32768,
            m if m.contains("gpt-4") => 8192,
            m if m.contains("gpt-3.5-turbo-16k") => 16384,
            m if m.contains("gpt-3.5") => 4096,
            m if m.contains("o1-preview") => 128000,
            m if m.contains("o1-mini") => 128000,
            _ => 4096, // Default
        }
    }
}

// Export for use in other modules
pub use OpenAIModel as OpenAI;