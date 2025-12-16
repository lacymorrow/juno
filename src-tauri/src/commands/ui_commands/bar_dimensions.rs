use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::constants::ui;

/// Window size configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSize {
    pub width: f64,
    pub height: f64,
}

impl WindowSize {
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

/// Bar-specific dimension configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarDimensions {
    pub default: WindowSize,
    pub states: HashMap<String, WindowSize>,
}

impl BarDimensions {
    /// Get dimensions for a specific state, falling back to default
    pub fn get_size_for_state(&self, state: &str) -> &WindowSize {
        self.states.get(state).unwrap_or(&self.default)
    }
}

/// Dimension configurations for all bar types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarDimensionConfig {
    pub floating: BarDimensions,
    pub app: BarDimensions,
    pub voice_ai: BarDimensions,
    pub dynamic: BarDimensions,
}

impl Default for BarDimensionConfig {
    fn default() -> Self {
        Self {
            floating: Self::default_floating_dimensions(),
            app: Self::default_app_dimensions(),
            voice_ai: Self::default_voice_ai_dimensions(),
            dynamic: Self::default_dynamic_dimensions(),
        }
    }
}

impl BarDimensionConfig {
    /// Get dimensions for a specific bar appearance and state
    pub fn get_dimensions(&self, appearance: &str, state: &str) -> WindowSize {
        let bar_dims = match appearance {
            ui::bar_appearances::APP => &self.app,
            ui::bar_appearances::VOICE_AI => &self.voice_ai,
            ui::bar_appearances::DYNAMIC => &self.dynamic,
            _ => &self.floating, // Default to floating
        };
        bar_dims.get_size_for_state(state).clone()
    }

    fn default_floating_dimensions() -> BarDimensions {
        let mut states = HashMap::new();
        
        // Define state-specific sizes for floating bar
        states.insert(ui::bar_states::DEFAULT.to_string(), WindowSize::new(60.0, 60.0));
        states.insert(ui::bar_states::EXPANDING.to_string(), WindowSize::new(60.0, 60.0));
        states.insert(ui::bar_states::INPUT.to_string(), WindowSize::new(600.0, 60.0));
        states.insert(ui::bar_states::SHRINKING.to_string(), WindowSize::new(60.0, 60.0));
        states.insert(ui::bar_states::SUBMITTING.to_string(), WindowSize::new(600.0, 60.0));
        states.insert(ui::bar_states::LOADING.to_string(), WindowSize::new(600.0, 60.0));
        states.insert(ui::bar_states::SUCCESS.to_string(), WindowSize::new(600.0, 60.0));
        states.insert(ui::bar_states::ERROR.to_string(), WindowSize::new(600.0, 60.0));
        states.insert(ui::bar_states::SPEAKING.to_string(), WindowSize::new(600.0, 60.0));
        states.insert(ui::bar_states::LISTENING.to_string(), WindowSize::new(600.0, 60.0));
        states.insert(ui::bar_states::TRANSCRIBING.to_string(), WindowSize::new(600.0, 60.0));
        states.insert(ui::bar_states::DICTATING.to_string(), WindowSize::new(600.0, 60.0));
        states.insert(ui::bar_states::AGENT_RESPONDING.to_string(), WindowSize::new(600.0, 60.0));

        BarDimensions {
            default: WindowSize::new(60.0, 60.0),
            states,
        }
    }

    fn default_app_dimensions() -> BarDimensions {
        let mut states = HashMap::new();
        
        // App bar has more consistent sizing
        states.insert(ui::bar_states::DEFAULT.to_string(), WindowSize::new(400.0, 50.0));
        states.insert(ui::bar_states::INPUT.to_string(), WindowSize::new(600.0, 50.0));
        states.insert(ui::bar_states::LOADING.to_string(), WindowSize::new(600.0, 50.0));
        states.insert(ui::bar_states::ERROR.to_string(), WindowSize::new(600.0, 50.0));

        BarDimensions {
            default: WindowSize::new(400.0, 50.0),
            states,
        }
    }

    fn default_voice_ai_dimensions() -> BarDimensions {
        let mut states = HashMap::new();
        
        // Voice AI bar dimensions
        states.insert(ui::bar_states::DEFAULT.to_string(), WindowSize::new(320.0, 80.0));
        states.insert(ui::bar_states::LISTENING.to_string(), WindowSize::new(350.0, 100.0));
        states.insert(ui::bar_states::TRANSCRIBING.to_string(), WindowSize::new(400.0, 120.0));
        states.insert(ui::bar_states::SPEAKING.to_string(), WindowSize::new(350.0, 100.0));
        states.insert(ui::bar_states::LOADING.to_string(), WindowSize::new(350.0, 90.0));

        BarDimensions {
            default: WindowSize::new(320.0, 80.0),
            states,
        }
    }

    fn default_dynamic_dimensions() -> BarDimensions {
        let mut states = HashMap::new();
        
        // Dynamic bar has many different sizes based on state
        states.insert(ui::bar_states::DEFAULT.to_string(), WindowSize::new(80.0, 30.0));
        states.insert(ui::bar_states::EXPANDING.to_string(), WindowSize::new(160.0, 40.0));
        states.insert(ui::bar_states::INPUT.to_string(), WindowSize::new(400.0, 60.0));
        states.insert(ui::bar_states::SUBMITTING.to_string(), WindowSize::new(200.0, 50.0));
        states.insert(ui::bar_states::LOADING.to_string(), WindowSize::new(200.0, 50.0));
        states.insert(ui::bar_states::SUCCESS.to_string(), WindowSize::new(180.0, 45.0));
        states.insert(ui::bar_states::ERROR.to_string(), WindowSize::new(250.0, 55.0));
        states.insert(ui::bar_states::LISTENING.to_string(), WindowSize::new(250.0, 80.0));
        states.insert(ui::bar_states::TRANSCRIBING.to_string(), WindowSize::new(280.0, 65.0));
        states.insert(ui::bar_states::SPEAKING.to_string(), WindowSize::new(160.0, 40.0));
        states.insert(ui::bar_states::AGENT_RESPONDING.to_string(), WindowSize::new(280.0, 65.0));

        BarDimensions {
            default: WindowSize::new(80.0, 30.0),
            states,
        }
    }
}