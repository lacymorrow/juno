// Anthropic Provider Implementation for Model Zoo
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use reqwest::Client;
use std::time::Duration;
use crate::agent::model_zoo::{ModelConfig, ModelInterface};

#[derive(Debug, Clone)]
pub struct AnthropicModel {
    config: ModelConfig,
    client: Client,
    api_key: String,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: usize,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Multimodal(Vec<ContentPart>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { 
        source: ImageSource,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct ImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    role: String,
    content: Vec<AnthropicResponseContent>,
    model: String,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponseContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: usize,
    output_tokens: usize,
}

impl AnthropicModel {
    pub async fn new(config: ModelConfig) -> Result<Self, String> {
        let api_key = config.api_key.clone()
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
            .ok_or_else(|| "Anthropic API key not found. Set ANTHROPIC_API_KEY environment variable.".to_string())?;
        
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
        // Extract model name from model_string (e.g., "anthropic/claude-3-5-sonnet" -> "claude-3-5-sonnet-20241022")
        let model = self.config.model_string
            .strip_prefix("anthropic/")
            .unwrap_or(&self.config.model_string);
        
        // Add version suffixes for models that need them
        match model {
            "claude-3-5-sonnet" => "claude-3-5-sonnet-20241022".to_string(),
            "claude-3-5-haiku" => "claude-3-5-haiku-20241022".to_string(),
            "claude-3-opus" => "claude-3-opus-20240229".to_string(),
            "claude-3-sonnet" => "claude-3-sonnet-20240229".to_string(),
            "claude-3-haiku" => "claude-3-haiku-20240307".to_string(),
            _ => model.to_string(),
        }
    }
    
    async fn make_request(&self, messages: Vec<AnthropicMessage>) -> Result<String, String> {
        let endpoint = self.config.endpoint.as_deref()
            .unwrap_or("https://api.anthropic.com/v1/messages");
        
        let request_body = AnthropicRequest {
            model: self.get_model_name(),
            messages,
            max_tokens: self.config.max_tokens.unwrap_or(4096),
            temperature: self.config.temperature,
            system: None,
        };
        
        let response = self.client
            .post(endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("Anthropic API error ({}): {}", status, error_text));
        }
        
        let anthropic_response: AnthropicResponse = response.json().await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        
        // Extract text from response
        let text = anthropic_response.content
            .into_iter()
            .filter_map(|c| c.text)
            .collect::<Vec<_>>()
            .join("");
        
        if text.is_empty() {
            return Err("Empty response from Anthropic API".to_string());
        }
        
        Ok(text)
    }
}

#[async_trait]
impl ModelInterface for AnthropicModel {
    async fn generate(&self, prompt: String, images: Option<Vec<Vec<u8>>>) -> Result<String, String> {
        let content = if let Some(images) = images {
            // Create multimodal content with text and images
            let mut parts = vec![ContentPart::Text { text: prompt }];
            
            for image_data in images {
                // Detect image format (assume PNG for now, could be improved)
                let base64_data = base64::encode(&image_data);
                parts.push(ContentPart::Image {
                    source: ImageSource {
                        source_type: "base64".to_string(),
                        media_type: "image/png".to_string(),
                        data: base64_data,
                    },
                });
            }
            
            AnthropicContent::Multimodal(parts)
        } else {
            AnthropicContent::Text(prompt)
        };
        
        let message = AnthropicMessage {
            role: "user".to_string(),
            content,
        };
        
        self.make_request(vec![message]).await
    }
    
    async fn stream_generate(&self, prompt: String) -> Result<tokio::sync::mpsc::Receiver<String>, String> {
        // Create a channel for streaming
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        
        let endpoint = self.config.endpoint.as_deref()
            .unwrap_or("https://api.anthropic.com/v1/messages");
        
        let message = AnthropicMessage {
            role: "user".to_string(),
            content: AnthropicContent::Text(prompt),
        };
        
        let mut request_body = AnthropicRequest {
            model: self.get_model_name(),
            messages: vec![message],
            max_tokens: self.config.max_tokens.unwrap_or(4096),
            temperature: self.config.temperature,
            system: None,
        };
        
        // Add stream flag
        let mut request_json = serde_json::to_value(&request_body)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;
        request_json["stream"] = Value::Bool(true);
        
        let api_key = self.api_key.clone();
        let client = self.client.clone();
        
        // Spawn async task to handle streaming
        tokio::spawn(async move {
            let response = match client
                .post(endpoint)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&request_json)
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
                                if let Ok(json) = serde_json::from_str::<Value>(data) {
                                    if let Some(delta) = json.get("delta") {
                                        if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                            if tx.send(text.to_string()).await.is_err() {
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
        // All Claude 3 models support vision
        model.contains("claude-3")
    }
    
    fn supports_tools(&self) -> bool {
        // All Claude models support function calling
        true
    }
    
    fn get_context_window(&self) -> usize {
        let model = self.get_model_name();
        match model.as_str() {
            m if m.contains("claude-3-5") => 200000,
            m if m.contains("claude-3-opus") => 200000,
            m if m.contains("claude-3-sonnet") => 200000,
            m if m.contains("claude-3-haiku") => 200000,
            m if m.contains("claude-2.1") => 200000,
            m if m.contains("claude-2") => 100000,
            m if m.contains("claude-instant") => 100000,
            _ => 100000, // Default
        }
    }
}

// Export for use in other modules
pub use AnthropicModel as Anthropic;