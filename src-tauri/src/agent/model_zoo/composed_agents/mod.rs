// Composed Agents - Combining UI grounding models with planning models
// Similar to CUA's omniparser+claude approach

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use super::{ModelInterface, ModelProvider, ModelConfig};

#[derive(Debug, Clone)]
pub struct ComposedAgent {
    ui_grounding_model: Box<dyn ModelInterface>,
    planning_model: Box<dyn ModelInterface>,
    config: ModelConfig,
}

impl ComposedAgent {
    pub async fn new(
        ui_provider: ModelProvider,
        planning_provider: ModelProvider,
        config: ModelConfig,
    ) -> Result<Self> {
        // Initialize both models
        let ui_config = ModelConfig {
            provider: ui_provider,
            ..config.clone()
        };
        
        let planning_config = ModelConfig {
            provider: planning_provider,
            ..config.clone()
        };
        
        let ui_grounding_model = super::ModelFactory::create(ui_config).await?;
        let planning_model = super::ModelFactory::create(planning_config).await?;
        
        Ok(Self {
            ui_grounding_model,
            planning_model,
            config,
        })
    }
    
    async fn ground_ui_elements(&self, screenshot: Vec<u8>) -> Result<String> {
        // Use UI grounding model to identify elements
        let prompt = "Identify all UI elements in this screenshot with their coordinates.";
        self.ui_grounding_model.generate(prompt.to_string(), Some(vec![screenshot])).await
    }
    
    async fn plan_action(&self, ui_elements: String, user_task: String) -> Result<String> {
        // Use planning model to determine actions
        let prompt = format!(
            "Given these UI elements:\n{}\n\nTask: {}\n\nProvide the next action.",
            ui_elements,
            user_task
        );
        self.planning_model.generate(prompt, None).await
    }
}

#[async_trait]
impl ModelInterface for ComposedAgent {
    async fn generate(&self, prompt: String, images: Option<Vec<Vec<u8>>>) -> Result<String> {
        // Composed generation: first ground UI, then plan
        if let Some(imgs) = &images {
            if !imgs.is_empty() {
                let ui_elements = self.ground_ui_elements(imgs[0].clone()).await?;
                return self.plan_action(ui_elements, prompt).await;
            }
        }
        
        // Fallback to planning model for non-visual tasks
        self.planning_model.generate(prompt, images).await
    }
    
    async fn stream_generate(&self, prompt: String) -> Result<tokio::sync::mpsc::Receiver<String>> {
        self.planning_model.stream_generate(prompt).await
    }
    
    fn supports_vision(&self) -> bool {
        true // Composed agents always support vision through UI grounding
    }
    
    fn supports_tools(&self) -> bool {
        self.planning_model.supports_tools()
    }
    
    fn get_context_window(&self) -> usize {
        self.planning_model.get_context_window()
    }
}