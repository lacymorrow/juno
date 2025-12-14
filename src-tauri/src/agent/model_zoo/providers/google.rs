// Google Provider Implementation for Model Zoo (Gemini models)
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use reqwest::Client;
use std::time::Duration;
use crate::agent::model_zoo::{ModelConfig, ModelInterface};

#[derive(Debug, Clone)]
pub struct GoogleModel {
    config: ModelConfig,
    client: Client,
    api_key: String,
}

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
    #[serde(rename = "safetySettings", skip_serializing_if = "Option::is_none")]
    safety_settings: Option<Vec<SafetySetting>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    parts: Vec<Part>,
    role: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Part {
    Text { text: String },
    InlineData { 
        #[serde(rename = "inlineData")]
        inline_data: InlineData 
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct InlineData {
    #[serde(rename = "mimeType")]
    mime_type: String,
    data: String,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens", skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<usize>,
    #[serde(rename = "topP", skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(rename = "topK", skip_serializing_if = "Option::is_none")]
    top_k: Option<i32>,
}

#[derive(Debug, Serialize)]
struct SafetySetting {
    category: String,
    threshold: String,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
    #[serde(rename = "usageMetadata", skip_serializing_if = "Option::is_none")]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: CandidateContent,
    #[serde(rename = "finishReason", skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CandidateContent {
    parts: Vec<ResponsePart>,
    role: String,
}

#[derive(Debug, Deserialize)]
struct ResponsePart {
    text: String,
}

#[derive(Debug, Deserialize)]
struct UsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: usize,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: usize,
    #[serde(rename = "totalTokenCount")]
    total_token_count: usize,
}

impl GoogleModel {
    pub async fn new(config: ModelConfig) -> Result<Self, String> {
        let api_key = config.api_key.clone()
            .or_else(|| std::env::var("GOOGLE_API_KEY").ok())
            .or_else(|| std::env::var("GEMINI_API_KEY").ok())
            .ok_or_else(|| "Google API key not found. Set GOOGLE_API_KEY or GEMINI_API_KEY environment variable.".to_string())?;
        
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
        // Extract model name from model_string (e.g., "google/gemini-2.0-flash" -> "gemini-2.0-flash-exp")
        let model = self.config.model_string
            .strip_prefix("google/")
            .unwrap_or(&self.config.model_string);
        
        // Add version suffixes or map to actual model names
        match model {
            "gemini-2.0-flash" => "gemini-2.0-flash-exp".to_string(),
            "gemini-2.0-flash-exp" => "gemini-2.0-flash-exp".to_string(),
            "gemini-1.5-pro" => "gemini-1.5-pro".to_string(),
            "gemini-1.5-flash" => "gemini-1.5-flash".to_string(),
            "gemini-1.5-flash-8b" => "gemini-1.5-flash-8b".to_string(),
            "gemini-pro" => "gemini-1.5-pro".to_string(), // Alias
            _ => model.to_string(),
        }
    }
    
    async fn make_request(&self, content: GeminiContent) -> Result<String, String> {
        let model_name = self.get_model_name();
        let endpoint = self.config.endpoint.as_deref()
            .unwrap_or(&format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent", model_name));
        
        let generation_config = GenerationConfig {
            temperature: self.config.temperature,
            max_output_tokens: self.config.max_tokens,
            top_p: None,
            top_k: None,
        };
        
        // Default safety settings (can be customized)
        let safety_settings = Some(vec![
            SafetySetting {
                category: "HARM_CATEGORY_HARASSMENT".to_string(),
                threshold: "BLOCK_ONLY_HIGH".to_string(),
            },
            SafetySetting {
                category: "HARM_CATEGORY_HATE_SPEECH".to_string(),
                threshold: "BLOCK_ONLY_HIGH".to_string(),
            },
            SafetySetting {
                category: "HARM_CATEGORY_SEXUALLY_EXPLICIT".to_string(),
                threshold: "BLOCK_ONLY_HIGH".to_string(),
            },
            SafetySetting {
                category: "HARM_CATEGORY_DANGEROUS_CONTENT".to_string(),
                threshold: "BLOCK_ONLY_HIGH".to_string(),
            },
        ]);
        
        let request_body = GeminiRequest {
            contents: vec![content],
            generation_config,
            safety_settings,
        };
        
        let url = format!("{}?key={}", endpoint, self.api_key);
        
        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("Google API error ({}): {}", status, error_text));
        }
        
        let gemini_response: GeminiResponse = response.json().await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        
        // Extract text from response
        let text = gemini_response.candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .map(|p| p.text.clone())
            .ok_or_else(|| "Empty response from Gemini API".to_string())?;
        
        Ok(text)
    }
}

#[async_trait]
impl ModelInterface for GoogleModel {
    async fn generate(&self, prompt: String, images: Option<Vec<Vec<u8>>>) -> Result<String, String> {
        let mut parts = vec![Part::Text { text: prompt }];
        
        // Add images if provided
        if let Some(images) = images {
            for image_data in images {
                // Detect image format (assume PNG for now, could be improved)
                let base64_data = base64::encode(&image_data);
                parts.push(Part::InlineData {
                    inline_data: InlineData {
                        mime_type: "image/png".to_string(),
                        data: base64_data,
                    },
                });
            }
        }
        
        let content = GeminiContent {
            parts,
            role: "user".to_string(),
        };
        
        self.make_request(content).await
    }
    
    async fn stream_generate(&self, prompt: String) -> Result<tokio::sync::mpsc::Receiver<String>, String> {
        // Create a channel for streaming
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        
        let model_name = self.get_model_name();
        let endpoint = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent", model_name);
        
        let content = GeminiContent {
            parts: vec![Part::Text { text: prompt }],
            role: "user".to_string(),
        };
        
        let generation_config = GenerationConfig {
            temperature: self.config.temperature,
            max_output_tokens: self.config.max_tokens,
            top_p: None,
            top_k: None,
        };
        
        let request_body = GeminiRequest {
            contents: vec![content],
            generation_config,
            safety_settings: None,
        };
        
        let url = format!("{}?key={}&alt=sse", endpoint, self.api_key);
        
        let api_key = self.api_key.clone();
        let client = self.client.clone();
        
        // Spawn async task to handle streaming
        tokio::spawn(async move {
            let response = match client
                .post(&url)
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
            
            let mut buffer = String::new();
            
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        // Parse SSE format
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);
                        
                        // Process complete messages
                        while let Some(line_end) = buffer.find("\n\n") {
                            let line = buffer.drain(..line_end + 2).collect::<String>();
                            
                            if line.starts_with("data: ") {
                                let data = &line[6..].trim();
                                
                                // Parse JSON and extract text
                                if let Ok(json) = serde_json::from_str::<Value>(data) {
                                    if let Some(candidates) = json.get("candidates") {
                                        if let Some(candidate) = candidates.get(0) {
                                            if let Some(content) = candidate.get("content") {
                                                if let Some(parts) = content.get("parts") {
                                                    if let Some(part) = parts.get(0) {
                                                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                                            if tx.send(text.to_string()).await.is_err() {
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }
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
        // All Gemini models support vision
        model.contains("gemini")
    }
    
    fn supports_tools(&self) -> bool {
        let model = self.get_model_name();
        // All Gemini 1.5+ models support function calling
        model.contains("gemini-1.5") || model.contains("gemini-2")
    }
    
    fn get_context_window(&self) -> usize {
        let model = self.get_model_name();
        match model.as_str() {
            m if m.contains("gemini-2.0-flash") => 1_000_000, // 1M tokens
            m if m.contains("gemini-1.5-pro") => 2_000_000,   // 2M tokens
            m if m.contains("gemini-1.5-flash") => 1_000_000, // 1M tokens
            m if m.contains("gemini-1.5-flash-8b") => 1_000_000, // 1M tokens
            _ => 32768, // Default (32K)
        }
    }
}

// Export for use in other modules
pub use GoogleModel as Google;