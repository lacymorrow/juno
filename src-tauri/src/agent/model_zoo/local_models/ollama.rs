// Ollama Local Model Provider Implementation
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use reqwest::Client;
use std::time::Duration;
use crate::agent::model_zoo::{ModelConfig, ModelInterface};

#[derive(Debug, Clone)]
pub struct OllamaModel {
    config: ModelConfig,
    client: Client,
    endpoint: String,
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    model: String,
    created_at: String,
    response: String,
    done: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaStreamResponse {
    model: String,
    created_at: String,
    response: String,
    done: bool,
}

impl OllamaModel {
    pub async fn new(config: ModelConfig) -> Result<Self, String> {
        let endpoint = config.endpoint.as_deref()
            .unwrap_or("http://localhost:11434")
            .to_string();
        
        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // Longer timeout for local models
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        
        // Check if Ollama is running
        let health_check = client
            .get(format!("{}/api/tags", endpoint))
            .send()
            .await;
        
        if health_check.is_err() {
            return Err(format!("Ollama is not running at {}. Please start Ollama first.", endpoint));
        }
        
        Ok(Self {
            config,
            client,
            endpoint,
        })
    }
    
    fn get_model_name(&self) -> String {
        // Extract model name from model_string (e.g., "ollama/llama3.2" -> "llama3.2")
        self.config.model_string
            .strip_prefix("ollama/")
            .unwrap_or(&self.config.model_string)
            .to_string()
    }
    
    async fn ensure_model_pulled(&self, model: &str) -> Result<(), String> {
        // Check if model exists, if not pull it
        let pull_endpoint = format!("{}/api/pull", self.endpoint);
        
        let pull_request = serde_json::json!({
            "name": model,
            "stream": false
        });
        
        let response = self.client
            .post(&pull_endpoint)
            .json(&pull_request)
            .send()
            .await
            .map_err(|e| format!("Failed to check/pull model: {}", e))?;
        
        if !response.status().is_success() {
            // Model might already exist, which is fine
            // Only error if it's a real error
            let status = response.status();
            if status.as_u16() != 409 { // 409 means model already exists
                let error_text = response.text().await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                return Err(format!("Failed to pull model ({}): {}", status, error_text));
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl ModelInterface for OllamaModel {
    async fn generate(&self, prompt: String, images: Option<Vec<Vec<u8>>>) -> Result<String, String> {
        let model = self.get_model_name();
        
        // Ensure model is available
        self.ensure_model_pulled(&model).await?;
        
        let endpoint = format!("{}/api/generate", self.endpoint);
        
        // Convert images to base64 if provided
        let image_strings = images.map(|imgs| {
            imgs.into_iter()
                .map(|img| base64::encode(&img))
                .collect()
        });
        
        let options = Some(OllamaOptions {
            temperature: self.config.temperature,
            num_predict: self.config.max_tokens,
        });
        
        let request_body = OllamaRequest {
            model,
            prompt,
            images: image_strings,
            options,
            stream: false,
        };
        
        let response = self.client
            .post(&endpoint)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("Ollama API error ({}): {}", status, error_text));
        }
        
        let ollama_response: OllamaResponse = response.json().await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        
        Ok(ollama_response.response)
    }
    
    async fn stream_generate(&self, prompt: String) -> Result<tokio::sync::mpsc::Receiver<String>, String> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        
        let model = self.get_model_name();
        let endpoint = format!("{}/api/generate", self.endpoint);
        
        let options = Some(OllamaOptions {
            temperature: self.config.temperature,
            num_predict: self.config.max_tokens,
        });
        
        let request_body = OllamaRequest {
            model,
            prompt,
            images: None,
            options,
            stream: true,
        };
        
        let client = self.client.clone();
        
        tokio::spawn(async move {
            let response = match client
                .post(&endpoint)
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
                        let text = String::from_utf8_lossy(&bytes);
                        // Parse NDJSON format
                        for line in text.lines() {
                            if line.is_empty() {
                                continue;
                            }
                            
                            if let Ok(response) = serde_json::from_str::<OllamaStreamResponse>(line) {
                                if !response.response.is_empty() {
                                    if tx.send(response.response).await.is_err() {
                                        break;
                                    }
                                }
                                
                                if response.done {
                                    break;
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
        // LLaVA and other vision models
        model.contains("llava") || model.contains("bakllava") || model.contains("moondream")
    }
    
    fn supports_tools(&self) -> bool {
        // Most modern Ollama models support function calling
        let model = self.get_model_name();
        model.contains("llama3") || 
        model.contains("qwen") || 
        model.contains("mistral") ||
        model.contains("gemma2")
    }
    
    fn get_context_window(&self) -> usize {
        let model = self.get_model_name();
        match model.as_str() {
            m if m.contains("llama3.2") => 128000,
            m if m.contains("llama3.1") => 128000,
            m if m.contains("llama3") => 8192,
            m if m.contains("qwen2.5-coder") => 32768,
            m if m.contains("qwen2.5") => 128000,
            m if m.contains("mistral") => 32768,
            m if m.contains("gemma2") => 8192,
            m if m.contains("deepseek-coder") => 16384,
            _ => 4096, // Default
        }
    }
}

// Export for use in other modules
pub use OllamaModel as Ollama;