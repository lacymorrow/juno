//! Shared provider types — the single source of truth for provider identity,
//! model IDs, and model metadata. No dependencies on config or factory.

// Model ID Constants - Single source of truth
pub mod model_ids {
    // Anthropic Claude Models — Current Generation
    pub const CLAUDE_FABLE_5: &str = "claude-fable-5";
    pub const CLAUDE_OPUS_4_8: &str = "claude-opus-4-8";
    pub const CLAUDE_OPUS_4_7: &str = "claude-opus-4-7";
    pub const CLAUDE_SONNET_4_6: &str = "claude-sonnet-4-6";
    pub const CLAUDE_OPUS_4_6: &str = "claude-opus-4-6";
    pub const CLAUDE_SONNET_4_5: &str = "claude-sonnet-4-5-20250929";
    pub const CLAUDE_HAIKU_4_5: &str = "claude-haiku-4-5-20251001";

    // Anthropic Claude Models — Legacy
    pub const CLAUDE_OPUS_4_5: &str = "claude-opus-4-5-20251101";
    pub const CLAUDE_OPUS_4_1: &str = "claude-opus-4-1-20250805";
    pub const CLAUDE_SONNET_4: &str = "claude-sonnet-4-20250514";
    pub const CLAUDE_OPUS_4: &str = "claude-opus-4-20250514";

    /// Models that require the 2025-11-24 computer-use beta flag AND
    /// new computer type (computer_20251124) + new editor (text_editor_20250728).
    /// These also support high-resolution screenshots up to 2,576px.
    pub const OPUS_4_5_PLUS_MODELS: &[&str] = &[
        CLAUDE_FABLE_5,
        CLAUDE_OPUS_4_8,
        CLAUDE_OPUS_4_5,
        CLAUDE_OPUS_4_6,
        CLAUDE_OPUS_4_7,
        CLAUDE_SONNET_4_6,
    ];

    /// Models that use the old computer type (computer_20250124) but require
    /// the new editor (text_editor_20250728). These sit between Opus 4.5+ and legacy.
    pub const MODELS_NEEDING_NEW_EDITOR: &[&str] = &[CLAUDE_SONNET_4_5, CLAUDE_HAIKU_4_5];

    // OpenAI Models
    pub const OPENAI_CUA: &str = "computer-use-preview";
    pub const OPENAI_CODEX_5_3: &str = "gpt-5.3-codex";

    // Google Gemini Models
    pub const GEMINI_2_5_COMPUTER_USE_PREVIEW: &str = "gemini-2.5-computer-use-preview-10-2025";
}

/// Model categories based on capabilities
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum ModelCategory {
    ComputerUse, // Models that support computer automation
    GeneralChat, // Models for general conversation and text generation
}

/// Model definition with all metadata
#[derive(Debug, Clone)]
pub struct ModelDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub category: ModelCategory,
    pub supports_computer_use: bool,
    pub is_recommended: bool,
}

/// Model information for serialization (UI display)
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub category: ModelCategory,
    pub supports_computer_use: bool,
    pub is_recommended: bool,
}

impl From<&ModelDefinition> for ModelInfo {
    fn from(def: &ModelDefinition) -> Self {
        ModelInfo {
            id: def.id.to_string(),
            name: def.name.to_string(),
            category: def.category.clone(),
            supports_computer_use: def.supports_computer_use,
            is_recommended: def.is_recommended,
        }
    }
}

/// Enumeration of available AI providers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    OpenAI,
    Rig,
    Gemini,
    /// Claude CLI (Claude Code) — subprocess-based provider, no API key needed.
    /// Uses the locally installed `claude` binary with the user's existing auth.
    ClaudeCli,
}

impl Provider {
    /// Convert a string to a Provider enum
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "anthropic" => Some(Provider::Anthropic),
            "openai" => Some(Provider::OpenAI),
            "rig" => Some(Provider::Rig),
            "gemini" => Some(Provider::Gemini),
            "claude_cli" | "claude-cli" | "claudecli" => Some(Provider::ClaudeCli),
            _ => None,
        }
    }

    /// Get display name for the provider
    pub fn display_name(&self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic Claude",
            Provider::OpenAI => "OpenAI GPT",
            Provider::Rig => "Rig AI Agent",
            Provider::Gemini => "Google Gemini",
            Provider::ClaudeCli => "Claude CLI",
        }
    }

    /// Get description for the provider
    pub fn description(&self) -> &'static str {
        match self {
            Provider::Anthropic => {
                "High-performance AI assistant with advanced reasoning capabilities"
            }
            Provider::OpenAI => "OpenAI's GPT models for conversational AI and text generation",
            Provider::Rig => "Rig framework for building AI agents with structured outputs",
            Provider::Gemini => "Google's Gemini models for multimodal AI capabilities",
            Provider::ClaudeCli => "Use your local Claude CLI installation — no API key required",
        }
    }

    /// Get the correct computer-use beta flag for the given model
    pub fn computer_use_beta_flag(&self, model: &str) -> &'static str {
        use crate::constants::api::beta_flags;

        match self {
            Provider::Anthropic => {
                if model_ids::OPUS_4_5_PLUS_MODELS.contains(&model) {
                    beta_flags::COMPUTER_USE_2025_11_24
                } else {
                    beta_flags::COMPUTER_USE_2025_01_24
                }
            }
            // Claude CLI handles its own API flags internally
            _ => "",
        }
    }

    /// Resolve the correct tool API type for the given model.
    ///
    /// Three tiers of Anthropic model behavior:
    /// - **Opus 4.5+**: new computer (`computer_20251124`) + new editor (`text_editor_20250728`)
    /// - **Sonnet 4.5 / Haiku 4.5**: old computer (`computer_20250124`) + new editor (`text_editor_20250728`)
    /// - **Legacy**: old computer + old editor (passthrough)
    pub fn resolve_tool_type(&self, tool_name: &str, registered_type: &str, model: &str) -> String {
        use crate::constants::api::computer_use_api_types;

        match self {
            Provider::Anthropic => {
                if model_ids::OPUS_4_5_PLUS_MODELS.contains(&model) {
                    // Tier 1: Opus 4.5+ — both computer and editor are new versions
                    match tool_name {
                        "computer" => {
                            return computer_use_api_types::COMPUTER_20251124.to_string()
                        }
                        "str_replace_based_edit_tool" => {
                            return computer_use_api_types::EDIT_TOOL_20250728.to_string()
                        }
                        _ => {}
                    }
                } else if model_ids::MODELS_NEEDING_NEW_EDITOR.contains(&model) {
                    // Tier 2: Sonnet 4.5, Haiku 4.5 — old computer, new editor
                    if tool_name == "str_replace_based_edit_tool" {
                        return computer_use_api_types::EDIT_TOOL_20250728.to_string();
                    }
                }
                // Tier 3: Legacy models — passthrough
                registered_type.to_string()
            }
            _ => registered_type.to_string(),
        }
    }

    /// Get model definitions for the provider
    pub fn model_definitions(&self) -> &'static [ModelDefinition] {
        match self {
            Provider::Anthropic => {
                &[
                    // Current generation
                    ModelDefinition {
                        id: model_ids::CLAUDE_FABLE_5,
                        name: "Claude Fable 5",
                        category: ModelCategory::ComputerUse,
                        supports_computer_use: true,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_4_8,
                        name: "Claude Opus 4.8",
                        category: ModelCategory::ComputerUse,
                        supports_computer_use: true,
                        is_recommended: true,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_4_7,
                        name: "Claude Opus 4.7",
                        category: ModelCategory::ComputerUse,
                        supports_computer_use: true,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_SONNET_4_6,
                        name: "Claude Sonnet 4.6",
                        category: ModelCategory::ComputerUse,
                        supports_computer_use: true,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_4_6,
                        name: "Claude Opus 4.6",
                        category: ModelCategory::ComputerUse,
                        supports_computer_use: true,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_SONNET_4_5,
                        name: "Claude Sonnet 4.5",
                        category: ModelCategory::ComputerUse,
                        supports_computer_use: true,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_HAIKU_4_5,
                        name: "Claude Haiku 4.5",
                        category: ModelCategory::ComputerUse,
                        supports_computer_use: true,
                        is_recommended: false,
                    },
                    // Legacy models
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_4_5,
                        name: "Claude Opus 4.5",
                        category: ModelCategory::ComputerUse,
                        supports_computer_use: true,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_4_1,
                        name: "Claude Opus 4.1",
                        category: ModelCategory::ComputerUse,
                        supports_computer_use: true,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_SONNET_4,
                        name: "Claude Sonnet 4",
                        category: ModelCategory::ComputerUse,
                        supports_computer_use: true,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_4,
                        name: "Claude Opus 4",
                        category: ModelCategory::ComputerUse,
                        supports_computer_use: true,
                        is_recommended: false,
                    },
                ]
            }
            Provider::OpenAI => &[
                ModelDefinition {
                    id: model_ids::OPENAI_CODEX_5_3,
                    name: "GPT-5.3 Codex",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: true,
                },
                ModelDefinition {
                    id: model_ids::OPENAI_CUA,
                    name: "Computer-Using Agent (CUA)",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: false,
                },
            ],
            Provider::Rig => &[
                ModelDefinition {
                    id: model_ids::OPENAI_CUA,
                    name: "Computer-Using Agent (CUA) via Rig",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: true,
                },
            ],
            Provider::Gemini => &[
                ModelDefinition {
                    id: model_ids::GEMINI_2_5_COMPUTER_USE_PREVIEW,
                    name: "Gemini 2.5 Computer Use (Preview)",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: true,
                },
            ],
            Provider::ClaudeCli => &[
                ModelDefinition {
                    id: "sonnet",
                    name: "Claude Sonnet (via CLI)",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: true,
                },
                ModelDefinition {
                    id: "opus",
                    name: "Claude Opus (via CLI)",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: false,
                },
                ModelDefinition {
                    id: "haiku",
                    name: "Claude Haiku (via CLI)",
                    category: ModelCategory::ComputerUse,
                    supports_computer_use: true,
                    is_recommended: false,
                },
            ],
        }
    }

    /// Get available models for the provider (derived from model definitions)
    pub fn models(&self) -> Vec<String> {
        self.model_definitions()
            .iter()
            .map(|def| def.id.to_string())
            .collect()
    }

    /// Check if a model supports computer use capabilities
    pub fn model_supports_computer_use(&self, model: &str) -> bool {
        self.model_definitions()
            .iter()
            .find(|def| def.id == model)
            .map(|def| def.supports_computer_use)
            .unwrap_or(false)
    }

    /// Get model category (ComputerUse or GeneralChat)
    pub fn get_model_category(&self, model: &str) -> ModelCategory {
        self.model_definitions()
            .iter()
            .find(|def| def.id == model)
            .map(|def| def.category.clone())
            .unwrap_or(ModelCategory::GeneralChat)
    }

    /// Get default model for the provider
    pub fn default_model(&self) -> &'static str {
        // Find the first recommended model, or fallback to the first model
        self.model_definitions()
            .iter()
            .find(|def| def.is_recommended)
            .or_else(|| self.model_definitions().first())
            .map(|def| def.id)
            .unwrap_or_else(|| {
                // Fallback constants if no definitions exist (shouldn't happen)
                match self {
                    Provider::Anthropic => model_ids::CLAUDE_OPUS_4_6,
                    Provider::OpenAI => model_ids::OPENAI_CUA,
                    Provider::Rig => model_ids::OPENAI_CUA,
                    Provider::Gemini => model_ids::GEMINI_2_5_COMPUTER_USE_PREVIEW,
                    Provider::ClaudeCli => "sonnet",
                }
            })
    }

    /// Get provider ID string
    pub fn id(&self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenAI => "openai",
            Provider::Rig => "rig",
            Provider::Gemini => "gemini",
            Provider::ClaudeCli => "claude_cli",
        }
    }

    /// Get detailed model information with capabilities (derived from model definitions)
    pub fn get_model_info(&self) -> Vec<ModelInfo> {
        self.model_definitions()
            .iter()
            .map(ModelInfo::from)
            .collect()
    }

    /// Check if provider supports computer use capabilities
    pub fn supports_computer_use(&self) -> bool {
        self.model_definitions()
            .iter()
            .any(|def| def.supports_computer_use)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_tool_type_opus_4_5_remaps_computer() {
        let result = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_OPUS_4_5,
        );
        assert_eq!(result, "computer_20251124");
    }

    #[test]
    fn test_resolve_tool_type_opus_4_5_remaps_editor() {
        let result = Provider::Anthropic.resolve_tool_type(
            "str_replace_based_edit_tool",
            "text_editor_20250429",
            model_ids::CLAUDE_OPUS_4_5,
        );
        assert_eq!(result, "text_editor_20250728");
    }

    #[test]
    fn test_resolve_tool_type_opus_4_6_remaps() {
        let computer = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_OPUS_4_6,
        );
        assert_eq!(computer, "computer_20251124");

        let editor = Provider::Anthropic.resolve_tool_type(
            "str_replace_based_edit_tool",
            "text_editor_20250429",
            model_ids::CLAUDE_OPUS_4_6,
        );
        assert_eq!(editor, "text_editor_20250728");
    }

    #[test]
    fn test_resolve_tool_type_older_model_passes_through() {
        let computer = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_SONNET_4,
        );
        assert_eq!(computer, "computer_20250124");

        let editor = Provider::Anthropic.resolve_tool_type(
            "str_replace_based_edit_tool",
            "text_editor_20250429",
            model_ids::CLAUDE_SONNET_4,
        );
        assert_eq!(editor, "text_editor_20250429");
    }

    #[test]
    fn test_resolve_tool_type_bash_unchanged_for_all_models() {
        let opus_45 = Provider::Anthropic.resolve_tool_type(
            "bash",
            "bash_20250124",
            model_ids::CLAUDE_OPUS_4_5,
        );
        assert_eq!(opus_45, "bash_20250124");

        let sonnet = Provider::Anthropic.resolve_tool_type(
            "bash",
            "bash_20250124",
            model_ids::CLAUDE_SONNET_4,
        );
        assert_eq!(sonnet, "bash_20250124");
    }

    #[test]
    fn test_resolve_tool_type_sonnet_4_5_remaps_editor_only() {
        // Sonnet 4.5 needs new editor but keeps old computer type
        let computer = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_SONNET_4_5,
        );
        assert_eq!(computer, "computer_20250124", "Sonnet 4.5 should keep old computer type");

        let editor = Provider::Anthropic.resolve_tool_type(
            "str_replace_based_edit_tool",
            "text_editor_20250429",
            model_ids::CLAUDE_SONNET_4_5,
        );
        assert_eq!(editor, "text_editor_20250728", "Sonnet 4.5 should use new editor type");
    }

    #[test]
    fn test_resolve_tool_type_haiku_4_5_remaps_editor_only() {
        // Haiku 4.5 needs new editor but keeps old computer type
        let computer = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_HAIKU_4_5,
        );
        assert_eq!(computer, "computer_20250124", "Haiku 4.5 should keep old computer type");

        let editor = Provider::Anthropic.resolve_tool_type(
            "str_replace_based_edit_tool",
            "text_editor_20250429",
            model_ids::CLAUDE_HAIKU_4_5,
        );
        assert_eq!(editor, "text_editor_20250728", "Haiku 4.5 should use new editor type");
    }

    #[test]
    fn test_resolve_tool_type_opus_4_8_remaps() {
        let computer = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_OPUS_4_8,
        );
        assert_eq!(computer, "computer_20251124");

        let editor = Provider::Anthropic.resolve_tool_type(
            "str_replace_based_edit_tool",
            "text_editor_20250429",
            model_ids::CLAUDE_OPUS_4_8,
        );
        assert_eq!(editor, "text_editor_20250728");
    }

    #[test]
    fn test_resolve_tool_type_fable_5_remaps() {
        let computer = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_FABLE_5,
        );
        assert_eq!(computer, "computer_20251124");

        let editor = Provider::Anthropic.resolve_tool_type(
            "str_replace_based_edit_tool",
            "text_editor_20250429",
            model_ids::CLAUDE_FABLE_5,
        );
        assert_eq!(editor, "text_editor_20250728");
    }

    #[test]
    fn test_resolve_tool_type_non_anthropic_passes_through() {
        let result = Provider::OpenAI.resolve_tool_type(
            "computer",
            "computer_20250124",
            "some-model",
        );
        assert_eq!(result, "computer_20250124");
    }
}
