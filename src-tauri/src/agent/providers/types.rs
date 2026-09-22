//! Shared provider types — the single source of truth for provider identity,
//! model IDs, and model metadata. No dependencies on config or factory.

use crate::agent::tools::tool_versioning::ApiVersion;

// Model ID Constants - Single source of truth
pub mod model_ids {
    // Anthropic Claude Models — Current Generation
    pub const CLAUDE_FABLE_5_1: &str = "claude-fable-5-1";
    pub const CLAUDE_FABLE_5: &str = "claude-fable-5";
    pub const CLAUDE_OPUS_5: &str = "claude-opus-5";
    pub const CLAUDE_OPUS_4_8: &str = "claude-opus-4-8";
    pub const CLAUDE_OPUS_4_7: &str = "claude-opus-4-7";
    pub const CLAUDE_SONNET_5: &str = "claude-sonnet-5";
    pub const CLAUDE_SONNET_4_6: &str = "claude-sonnet-4-6";
    pub const CLAUDE_OPUS_4_6: &str = "claude-opus-4-6";
    pub const CLAUDE_SONNET_4_5: &str = "claude-sonnet-4-5-20250929";
    pub const CLAUDE_HAIKU_4_5: &str = "claude-haiku-4-5-20251001";

    // Anthropic Claude Models — Legacy (deprecated but still callable)
    // NOTE: Only models the Anthropic API still serves belong here. Opus 4.1
    // (retired 2026-08-05), Opus 4, and Sonnet 4 (both retired 2026-06-15) were
    // removed — requests to retired models return 404. Verify against the live
    // deprecations page before adding an ID back:
    // https://platform.claude.com/docs/en/about-claude/model-deprecations
    pub const CLAUDE_OPUS_4_5: &str = "claude-opus-4-5-20251101";

    // NOTE: computer-use tiering is NOT a list of model IDs here. Each model
    // declares what it supports in its own `ModelDefinition` (`computer_use`,
    // `availability`, `image_tier`), and the beta flag, tool types, screenshot
    // resolution, picker gating, picker mark, and system prompt all derive from
    // those fields. Adding a model means filling them in, not editing lists
    // scattered across the codebase — that drift is what this replaced.

    // OpenAI Models
    // `gpt-5.6-sol` is the migration target OpenAI names for the retired
    // computer-use-preview model, and its model page lists `computer_use`
    // under supported tools.
    pub const OPENAI_SOL_5_6: &str = "gpt-5.6-sol";
    pub const OPENAI_CODEX_5_3: &str = "gpt-5.3-codex";
    /// Shut down 2026-07-23. Kept only so an existing setting still resolves
    /// to a named model instead of an opaque ID.
    pub const OPENAI_CUA: &str = "computer-use-preview";

    // Google Gemini Models — the "Model versions" list at
    // https://ai.google.dev/gemini-api/docs/computer-use
    pub const GEMINI_3_8_FLASH: &str = "gemini-3.8-flash";
    pub const GEMINI_3_7_FLASH: &str = "gemini-3.7-flash";
    pub const GEMINI_3_5_FLASH: &str = "gemini-3.5-flash";
    pub const GEMINI_3_5_FLASH_LITE: &str = "gemini-3.5-flash-lite";
    pub const GEMINI_3_FLASH_PREVIEW: &str = "gemini-3-flash-preview";
    pub const GEMINI_2_5_COMPUTER_USE_PREVIEW: &str = "gemini-2.5-computer-use-preview-10-2025";
}

/// Model categories based on capabilities
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum ModelCategory {
    ComputerUse, // Models that support computer automation
    GeneralChat, // Models for general conversation and text generation
}

/// What a model can do with the computer, declared once per model.
///
/// The computer-use beta flag, the tool type strings sent to the API, whether
/// computer-use tools are registered at all, the "Chat only" mark in the model
/// picker, and the chat-only system-prompt addendum all derive from this.
/// (Screenshot resolution does not — that is `ImageTier`, a separate axis.)
///
/// Every value is verified against the provider's official documentation —
/// see the catalog in `Provider::model_definitions` for the per-model citation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComputerUse {
    /// Chat only. The model cannot see the screen or drive input, so its
    /// requests carry no computer-use tools and no computer-use beta flag.
    No,
    /// Supported through Anthropic's built-in computer-use tools at this
    /// tool version, which also selects the matching beta flag.
    AnthropicTool(ApiVersion),
    /// Supported through Juno's own function tools (OpenAI, Gemini, Rig) or
    /// the `juno-cua` MCP server (Claude CLI). Capable, but no Anthropic tool
    /// version applies, so no Anthropic beta flag is sent.
    FunctionTools,
}

impl ComputerUse {
    /// Whether the model can drive the computer at all.
    pub fn is_supported(&self) -> bool {
        !matches!(self, ComputerUse::No)
    }

    /// The Anthropic computer-use tool version this model takes, if any.
    pub fn anthropic_version(&self) -> Option<&ApiVersion> {
        match self {
            ComputerUse::AnthropicTool(version) => Some(version),
            _ => None,
        }
    }
}

/// Whether Juno offers a model by default, or keeps it for people who
/// deliberately go looking.
///
/// "Current" means the model is GA on Anthropic's current computer-use toolset
/// (`computer_toolset_20260801`), per the `supportedModels` list at
/// <https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool>.
/// Everything else drives the computer only through an earlier tool version and
/// is hidden unless advanced settings are on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Availability {
    Current,
    Legacy,
}

/// The image size a model accepts, which sets the screenshot resolution Juno
/// captures at.
///
/// Source: the resolution tier table at
/// <https://platform.claude.com/docs/en/build-with-claude/vision#evaluate-image-size>
/// — "High-resolution | Claude 4.7 and later models | 2576 px | 4784" and
/// "Standard | All other models | 1568 px | 1568". Note this does NOT line up
/// with the computer-use tool version: Opus 4.6 and Opus 4.5 take the same
/// `computer_20251124` tool as Opus 4.7 but sit on the standard image tier.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum ImageTier {
    /// 2576 px long edge, 4784 visual tokens. Claude 4.7 and later.
    HighResolution,
    /// 1568 px long edge. Every other model.
    Standard,
}

/// Model definition with all metadata.
///
/// These three capability fields are the one association table. Nothing else in
/// the codebase may keep its own list of model IDs to answer these questions.
#[derive(Debug, Clone)]
pub struct ModelDefinition {
    pub id: &'static str,
    pub name: &'static str,
    /// The computer-use tool version this model actually supports, or
    /// `ComputerUse::No` for a chat-only model.
    pub computer_use: ComputerUse,
    /// Whether the model is offered by default or only under advanced settings.
    pub availability: Availability,
    /// The image size tier, which decides screenshot capture resolution.
    pub image_tier: ImageTier,
    /// Accepts `thinking: {type: "adaptive"}`. Older models require
    /// `budget_tokens` and reject `adaptive` with a 400, so the provider omits
    /// the parameter when this is false.
    pub adaptive_thinking: bool,
    /// Accepts the server-side `fallbacks` parameter (beta
    /// `server-side-fallback-2026-07-01`), so a safety refusal is retried on a
    /// fallback model in the same round trip.
    pub server_side_fallback: bool,
    pub is_recommended: bool,
}

impl ModelDefinition {
    /// Whether this model can drive the computer. Derived, never stored —
    /// a stored copy is how the catalog drifted into claiming every model
    /// supports computer use.
    pub fn supports_computer_use(&self) -> bool {
        self.computer_use.is_supported()
    }

    /// The category this model falls into, derived from its capability.
    pub fn category(&self) -> ModelCategory {
        if self.supports_computer_use() {
            ModelCategory::ComputerUse
        } else {
            ModelCategory::GeneralChat
        }
    }
}

/// Model information for serialization (UI display)
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub category: ModelCategory,
    pub supports_computer_use: bool,
    pub is_recommended: bool,
    /// True when the model only drives the computer through an older tool
    /// version. The picker hides these unless advanced settings are on.
    pub is_legacy: bool,
}

impl From<&ModelDefinition> for ModelInfo {
    fn from(def: &ModelDefinition) -> Self {
        ModelInfo {
            id: def.id.to_string(),
            name: def.name.to_string(),
            category: def.category(),
            supports_computer_use: def.supports_computer_use(),
            is_recommended: def.is_recommended,
            is_legacy: def.availability == Availability::Legacy,
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

    /// What `model` can do with the computer, as declared in the catalog.
    ///
    /// An unknown model is treated as chat-only: Juno would rather refuse with
    /// a clear message than send computer-use tools a model will reject.
    pub fn computer_use(&self, model: &str) -> ComputerUse {
        self.model_definitions()
            .iter()
            .find(|def| def.id == model)
            .map(|def| def.computer_use.clone())
            .unwrap_or(ComputerUse::No)
    }

    /// The computer-use beta flag for `model`, or `None` when the model sends
    /// no computer-use tools (chat-only) or needs no Anthropic beta flag.
    pub fn computer_use_beta_flag(&self, model: &str) -> Option<&'static str> {
        self.computer_use(model)
            .anthropic_version()
            .map(|version| version.beta_flag())
    }

    /// Resolve the correct tool API type for the given model.
    ///
    /// The model's declared computer-use version decides this: a model that
    /// takes `computer_20251124` gets that computer type, a model that takes
    /// `computer_20250124` gets that one, and anything Juno does not have a
    /// declaration for keeps the type the tool registered with.
    pub fn resolve_tool_type(&self, tool_name: &str, registered_type: &str, model: &str) -> String {
        match self {
            Provider::Anthropic => match self.computer_use(model).anthropic_version() {
                Some(version) => match tool_name {
                    "computer" => version.computer_tool_type().to_string(),
                    "str_replace_based_edit_tool" => version.editor_tool_type().to_string(),
                    // bash is the same type across every version Juno supports.
                    _ => registered_type.to_string(),
                },
                None => registered_type.to_string(),
            },
            _ => registered_type.to_string(),
        }
    }

    /// Get model definitions for the provider.
    ///
    /// This is the one association table. Every per-model question the codebase
    /// asks — computer-use tool version, computer-use beta flag, editor tool
    /// type, screenshot resolution tier, adaptive thinking, server-side
    /// fallbacks, whether the picker shows it by default — is answered from a
    /// field here. There are no `&[&str]` model lists anywhere else, because
    /// four of them drifted out of agreement and that is what this replaced.
    ///
    /// Every value is verified against official documentation:
    ///
    /// - computer-use tool version and toolset GA list:
    ///   <https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool>
    /// - image tier:
    ///   <https://platform.claude.com/docs/en/build-with-claude/vision#evaluate-image-size>
    /// - Gemini computer-use models:
    ///   <https://ai.google.dev/gemini-api/docs/computer-use>
    /// - OpenAI per-model supported tools:
    ///   <https://developers.openai.com/api/docs/models>
    ///
    /// Do not edit these from memory. Re-read those pages.
    pub fn model_definitions(&self) -> &'static [ModelDefinition] {
        // The two Anthropic computer-use tool versions Juno sends today.
        const CU_20251124: ComputerUse = ComputerUse::AnthropicTool(ApiVersion::Computer20251124);
        const CU_20250124: ComputerUse = ComputerUse::AnthropicTool(ApiVersion::Computer20250124);

        match self {
            Provider::Anthropic => {
                &[
                    // --- Current: GA on computer_toolset_20260801 ---
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_5,
                        name: "Claude Opus 5",
                        computer_use: CU_20251124,
                        availability: Availability::Current,
                        image_tier: ImageTier::HighResolution,
                        adaptive_thinking: true,
                        server_side_fallback: true,
                        is_recommended: true,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_FABLE_5_1,
                        name: "Claude Fable 5.1",
                        computer_use: CU_20251124,
                        availability: Availability::Current,
                        image_tier: ImageTier::HighResolution,
                        adaptive_thinking: true,
                        server_side_fallback: true,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_FABLE_5,
                        name: "Claude Fable 5",
                        computer_use: CU_20251124,
                        availability: Availability::Current,
                        image_tier: ImageTier::HighResolution,
                        adaptive_thinking: true,
                        server_side_fallback: true,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_SONNET_5,
                        name: "Claude Sonnet 5",
                        computer_use: CU_20251124,
                        availability: Availability::Current,
                        image_tier: ImageTier::HighResolution,
                        adaptive_thinking: true,
                        server_side_fallback: false,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_4_8,
                        name: "Claude Opus 4.8",
                        computer_use: CU_20251124,
                        availability: Availability::Current,
                        image_tier: ImageTier::HighResolution,
                        adaptive_thinking: true,
                        server_side_fallback: false,
                        is_recommended: false,
                    },
                    // --- Legacy: computer use only through an earlier tool
                    // version, so these are advanced-only. Opus 4.7 is the one
                    // legacy model on the high-resolution image tier.
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_4_7,
                        name: "Claude Opus 4.7",
                        computer_use: CU_20251124,
                        availability: Availability::Legacy,
                        image_tier: ImageTier::HighResolution,
                        adaptive_thinking: true,
                        server_side_fallback: false,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_4_6,
                        name: "Claude Opus 4.6",
                        computer_use: CU_20251124,
                        availability: Availability::Legacy,
                        image_tier: ImageTier::Standard,
                        adaptive_thinking: true,
                        server_side_fallback: false,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_SONNET_4_6,
                        name: "Claude Sonnet 4.6",
                        computer_use: CU_20251124,
                        availability: Availability::Legacy,
                        image_tier: ImageTier::Standard,
                        adaptive_thinking: true,
                        server_side_fallback: false,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_4_5,
                        name: "Claude Opus 4.5",
                        computer_use: CU_20251124,
                        availability: Availability::Legacy,
                        image_tier: ImageTier::Standard,
                        adaptive_thinking: false,
                        server_side_fallback: false,
                        is_recommended: false,
                    },
                    // Sonnet 4.5 and Haiku 4.5 do support computer use, on the
                    // older computer_20250124 tool version. They are not chat
                    // only; they are just further behind.
                    ModelDefinition {
                        id: model_ids::CLAUDE_SONNET_4_5,
                        name: "Claude Sonnet 4.5",
                        computer_use: CU_20250124,
                        availability: Availability::Legacy,
                        image_tier: ImageTier::Standard,
                        adaptive_thinking: false,
                        server_side_fallback: false,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_HAIKU_4_5,
                        name: "Claude Haiku 4.5",
                        computer_use: CU_20250124,
                        availability: Availability::Legacy,
                        image_tier: ImageTier::Standard,
                        adaptive_thinking: false,
                        server_side_fallback: false,
                        is_recommended: false,
                    },
                ]
            }
            // Juno drives OpenAI and Gemini with its own function tools and
            // screenshots rather than those providers' native computer tools,
            // so what a model needs here is image input plus function calling.
            // No Anthropic tool version or image tier applies.
            Provider::OpenAI => &[
                ModelDefinition {
                    id: model_ids::OPENAI_SOL_5_6,
                    name: "GPT-5.6 Sol",
                    computer_use: ComputerUse::FunctionTools,
                    availability: Availability::Current,
                    image_tier: ImageTier::Standard,
                    adaptive_thinking: false,
                    server_side_fallback: false,
                    is_recommended: true,
                },
                ModelDefinition {
                    id: model_ids::OPENAI_CODEX_5_3,
                    name: "GPT-5.3 Codex",
                    computer_use: ComputerUse::FunctionTools,
                    availability: Availability::Current,
                    image_tier: ImageTier::Standard,
                    adaptive_thinking: false,
                    server_side_fallback: false,
                    is_recommended: false,
                },
                // Shut down by OpenAI on 2026-07-23. Hidden unless
                // advanced settings are on, so a saved setting still names it
                // rather than failing as an unknown model.
                ModelDefinition {
                    id: model_ids::OPENAI_CUA,
                    name: "Computer-Using Agent (CUA, retired)",
                    computer_use: ComputerUse::FunctionTools,
                    availability: Availability::Legacy,
                    image_tier: ImageTier::Standard,
                    adaptive_thinking: false,
                    server_side_fallback: false,
                    is_recommended: false,
                },
            ],
            Provider::Rig => &[ModelDefinition {
                id: model_ids::OPENAI_SOL_5_6,
                name: "GPT-5.6 Sol via Rig",
                computer_use: ComputerUse::FunctionTools,
                availability: Availability::Current,
                image_tier: ImageTier::Standard,
                adaptive_thinking: false,
                server_side_fallback: false,
                is_recommended: true,
            }],
            Provider::Gemini => &[
                ModelDefinition {
                    id: model_ids::GEMINI_3_8_FLASH,
                    name: "Gemini 3.8 Flash",
                    computer_use: ComputerUse::FunctionTools,
                    availability: Availability::Current,
                    image_tier: ImageTier::Standard,
                    adaptive_thinking: false,
                    server_side_fallback: false,
                    is_recommended: true,
                },
                ModelDefinition {
                    id: model_ids::GEMINI_3_7_FLASH,
                    name: "Gemini 3.7 Flash",
                    computer_use: ComputerUse::FunctionTools,
                    availability: Availability::Current,
                    image_tier: ImageTier::Standard,
                    adaptive_thinking: false,
                    server_side_fallback: false,
                    is_recommended: false,
                },
                ModelDefinition {
                    id: model_ids::GEMINI_3_5_FLASH,
                    name: "Gemini 3.5 Flash",
                    computer_use: ComputerUse::FunctionTools,
                    availability: Availability::Current,
                    image_tier: ImageTier::Standard,
                    adaptive_thinking: false,
                    server_side_fallback: false,
                    is_recommended: false,
                },
                ModelDefinition {
                    id: model_ids::GEMINI_3_5_FLASH_LITE,
                    name: "Gemini 3.5 Flash-Lite",
                    computer_use: ComputerUse::FunctionTools,
                    availability: Availability::Current,
                    image_tier: ImageTier::Standard,
                    adaptive_thinking: false,
                    server_side_fallback: false,
                    is_recommended: false,
                },
                ModelDefinition {
                    id: model_ids::GEMINI_3_FLASH_PREVIEW,
                    name: "Gemini 3 Flash Preview",
                    computer_use: ComputerUse::FunctionTools,
                    availability: Availability::Current,
                    image_tier: ImageTier::Standard,
                    adaptive_thinking: false,
                    server_side_fallback: false,
                    is_recommended: false,
                },
                // Google labels this one "Legacy Preview".
                ModelDefinition {
                    id: model_ids::GEMINI_2_5_COMPUTER_USE_PREVIEW,
                    name: "Gemini 2.5 Computer Use (Legacy Preview)",
                    computer_use: ComputerUse::FunctionTools,
                    availability: Availability::Legacy,
                    image_tier: ImageTier::Standard,
                    adaptive_thinking: false,
                    server_side_fallback: false,
                    is_recommended: false,
                },
            ],
            // The CLI drives the desktop through the juno-cua MCP server rather
            // than Anthropic's built-in computer tool, so it picks its own tool
            // versions and Juno sends no Anthropic beta flag. The aliases track
            // the current generation, which is the high-resolution image tier.
            Provider::ClaudeCli => &[
                ModelDefinition {
                    id: "sonnet",
                    name: "Claude Sonnet (via CLI)",
                    computer_use: ComputerUse::FunctionTools,
                    availability: Availability::Current,
                    image_tier: ImageTier::HighResolution,
                    adaptive_thinking: false,
                    server_side_fallback: false,
                    is_recommended: true,
                },
                ModelDefinition {
                    id: "opus",
                    name: "Claude Opus (via CLI)",
                    computer_use: ComputerUse::FunctionTools,
                    availability: Availability::Current,
                    image_tier: ImageTier::HighResolution,
                    adaptive_thinking: false,
                    server_side_fallback: false,
                    is_recommended: false,
                },
                ModelDefinition {
                    id: "haiku",
                    name: "Claude Haiku (via CLI)",
                    computer_use: ComputerUse::FunctionTools,
                    availability: Availability::Current,
                    image_tier: ImageTier::Standard,
                    adaptive_thinking: false,
                    server_side_fallback: false,
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
        self.computer_use(model).is_supported()
    }

    /// Whether `model` appears in this provider's catalog at all. A configured
    /// model is used verbatim, so settings can still name one Juno has dropped.
    pub fn knows_model(&self, model: &str) -> bool {
        self.model_definitions().iter().any(|def| def.id == model)
    }

    /// The image tier `model` accepts. Unknown models get the standard tier:
    /// too small is a worse screenshot, too big is a rejected request.
    pub fn image_tier(&self, model: &str) -> ImageTier {
        self.model_definitions()
            .iter()
            .find(|def| def.id == model)
            .map(|def| def.image_tier.clone())
            .unwrap_or(ImageTier::Standard)
    }

    /// Get model category (ComputerUse or GeneralChat)
    pub fn get_model_category(&self, model: &str) -> ModelCategory {
        if self.model_supports_computer_use(model) {
            ModelCategory::ComputerUse
        } else {
            ModelCategory::GeneralChat
        }
    }

    /// Why `model` cannot drive the computer, phrased for the person using it.
    /// `None` when the model can.
    pub fn computer_use_refusal(&self, model: &str) -> Option<String> {
        if self.model_supports_computer_use(model) {
            return None;
        }
        let name = self
            .model_definitions()
            .iter()
            .find(|def| def.id == model)
            .map(|def| def.name)
            .unwrap_or(model);
        Some(if self.knows_model(model) {
            format!(
                "{} is a chat-only model and cannot control the computer. \
                 Switch to a computer-use model in Settings > AI Provider.",
                name
            )
        } else {
            format!(
                "{} is not a model Juno can drive the computer with. \
                 Pick a current model in Settings > AI Provider.",
                name
            )
        })
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
                    Provider::Anthropic => model_ids::CLAUDE_FABLE_5_1,
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
    /// Whether `model` accepts `thinking: {type: "adaptive"}` on this provider.
    pub fn supports_adaptive_thinking(&self, model: &str) -> bool {
        matches!(self, Provider::Anthropic)
            && self
                .model_definitions()
                .iter()
                .any(|def| def.id == model && def.adaptive_thinking)
    }

    /// Whether `model` accepts the server-side `fallbacks` parameter on this provider.
    pub fn supports_server_side_fallbacks(&self, model: &str) -> bool {
        matches!(self, Provider::Anthropic)
            && self
                .model_definitions()
                .iter()
                .any(|def| def.id == model && def.server_side_fallback)
    }

    /// Whether this provider offers at least one computer-use capable model.
    pub fn supports_computer_use(&self) -> bool {
        self.model_definitions()
            .iter()
            .any(|def| def.supports_computer_use())
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
        // A pre-4.5 legacy model that is in neither special tier array must fall
        // through to Tier 3 (passthrough). Uses a literal retired ID because no
        // active model exercises this branch anymore — it guards the fallthrough
        // for older/unknown models generally.
        let legacy_model = "claude-3-5-sonnet-20241022";

        let computer =
            Provider::Anthropic.resolve_tool_type("computer", "computer_20250124", legacy_model);
        assert_eq!(computer, "computer_20250124");

        let editor = Provider::Anthropic.resolve_tool_type(
            "str_replace_based_edit_tool",
            "text_editor_20250429",
            legacy_model,
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

        // A legacy passthrough model must also leave bash untouched.
        let legacy = Provider::Anthropic.resolve_tool_type(
            "bash",
            "bash_20250124",
            "claude-3-5-sonnet-20241022",
        );
        assert_eq!(legacy, "bash_20250124");
    }

    #[test]
    fn test_resolve_tool_type_sonnet_4_5_remaps_editor_only() {
        // Sonnet 4.5 needs new editor but keeps old computer type
        let computer = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_SONNET_4_5,
        );
        assert_eq!(
            computer, "computer_20250124",
            "Sonnet 4.5 should keep old computer type"
        );

        let editor = Provider::Anthropic.resolve_tool_type(
            "str_replace_based_edit_tool",
            "text_editor_20250429",
            model_ids::CLAUDE_SONNET_4_5,
        );
        assert_eq!(
            editor, "text_editor_20250728",
            "Sonnet 4.5 should use new editor type"
        );
    }

    #[test]
    fn test_resolve_tool_type_haiku_4_5_remaps_editor_only() {
        // Haiku 4.5 needs new editor but keeps old computer type
        let computer = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_HAIKU_4_5,
        );
        assert_eq!(
            computer, "computer_20250124",
            "Haiku 4.5 should keep old computer type"
        );

        let editor = Provider::Anthropic.resolve_tool_type(
            "str_replace_based_edit_tool",
            "text_editor_20250429",
            model_ids::CLAUDE_HAIKU_4_5,
        );
        assert_eq!(
            editor, "text_editor_20250728",
            "Haiku 4.5 should use new editor type"
        );
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
    fn test_resolve_tool_type_sonnet_5_remaps() {
        let computer = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_SONNET_5,
        );
        assert_eq!(computer, "computer_20251124");

        let editor = Provider::Anthropic.resolve_tool_type(
            "str_replace_based_edit_tool",
            "text_editor_20250429",
            model_ids::CLAUDE_SONNET_5,
        );
        assert_eq!(editor, "text_editor_20250728");
    }

    #[test]
    fn test_resolve_tool_type_opus_5_remaps() {
        let computer = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_OPUS_5,
        );
        assert_eq!(computer, "computer_20251124");

        let editor = Provider::Anthropic.resolve_tool_type(
            "str_replace_based_edit_tool",
            "text_editor_20250429",
            model_ids::CLAUDE_OPUS_5,
        );
        assert_eq!(editor, "text_editor_20250728");
    }

    #[test]
    fn test_opus_5_is_default_anthropic_model() {
        // Juno defaults to a current, top-tier model. Fable 5.1 stays in the
        // catalog for work that needs the most capable model Anthropic ships,
        // but the default is Opus 5.
        assert_eq!(
            Provider::Anthropic.default_model(),
            model_ids::CLAUDE_OPUS_5
        );

        // Whatever the default is, it must be a model the picker shows without
        // advanced settings — otherwise a fresh install starts on a hidden model.
        let default = Provider::Anthropic
            .model_definitions()
            .iter()
            .find(|def| def.id == Provider::Anthropic.default_model())
            .expect("the default model must be in the catalog");
        assert_eq!(default.availability, Availability::Current);
    }

    #[test]
    fn test_resolve_tool_type_fable_5_1_remaps() {
        let computer = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_FABLE_5_1,
        );
        assert_eq!(computer, "computer_20251124");
        assert_eq!(
            Provider::Anthropic.computer_use_beta_flag(model_ids::CLAUDE_FABLE_5_1),
            Some(crate::constants::api::beta_flags::COMPUTER_USE_2025_11_24)
        );
    }

    #[test]
    fn test_adaptive_thinking_and_fallback_capabilities() {
        assert!(Provider::Anthropic.supports_adaptive_thinking(model_ids::CLAUDE_FABLE_5_1));
        assert!(Provider::Anthropic.supports_adaptive_thinking(model_ids::CLAUDE_OPUS_5));
        assert!(!Provider::Anthropic.supports_adaptive_thinking(model_ids::CLAUDE_HAIKU_4_5));
        assert!(!Provider::Anthropic.supports_adaptive_thinking(model_ids::CLAUDE_OPUS_4_5));
        assert!(Provider::Anthropic.supports_server_side_fallbacks(model_ids::CLAUDE_FABLE_5_1));
        assert!(!Provider::Anthropic.supports_server_side_fallbacks(model_ids::CLAUDE_OPUS_4_8));
        assert!(!Provider::OpenAI.supports_adaptive_thinking(model_ids::CLAUDE_FABLE_5_1));
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
        let result =
            Provider::OpenAI.resolve_tool_type("computer", "computer_20250124", "some-model");
        assert_eq!(result, "computer_20250124");
    }

    // --- capability declaration: the single source of truth ---

    /// The catalog must match Anthropic's docs on all three axes. If Anthropic
    /// moves a model, this is the test that should fail.
    ///
    /// Tool version: <https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool#earlier-tool-versions>
    /// Toolset GA:   the `supportedModels` list on that same page
    /// Image tier:   <https://platform.claude.com/docs/en/build-with-claude/vision#evaluate-image-size>
    #[test]
    fn anthropic_catalog_matches_the_documented_tables() {
        // The docs list exactly these two under `computer_20250124`; every
        // other model Juno offers is listed under `computer_20251124`.
        let earlier_tool = [model_ids::CLAUDE_SONNET_4_5, model_ids::CLAUDE_HAIKU_4_5];

        // GA on computer_toolset_20260801, intersected with what Juno offers.
        // (The docs also list claude-mythos-5 and claude-mythos-5-1, which are
        // access-restricted and not in Juno's catalog.)
        let toolset_ga = [
            model_ids::CLAUDE_FABLE_5_1,
            model_ids::CLAUDE_FABLE_5,
            model_ids::CLAUDE_OPUS_5,
            model_ids::CLAUDE_SONNET_5,
            model_ids::CLAUDE_OPUS_4_8,
        ];

        // "High-resolution | Claude 4.7 and later models". Opus 4.7 is legacy
        // on tool version but high-resolution on image tier.
        let high_res = [
            model_ids::CLAUDE_FABLE_5_1,
            model_ids::CLAUDE_FABLE_5,
            model_ids::CLAUDE_OPUS_5,
            model_ids::CLAUDE_SONNET_5,
            model_ids::CLAUDE_OPUS_4_8,
            model_ids::CLAUDE_OPUS_4_7,
        ];

        for def in Provider::Anthropic.model_definitions() {
            let expected_tool = if earlier_tool.contains(&def.id) {
                ApiVersion::Computer20250124
            } else {
                ApiVersion::Computer20251124
            };
            assert_eq!(
                def.computer_use,
                ComputerUse::AnthropicTool(expected_tool),
                "{} declares the wrong computer-use tool version",
                def.name
            );

            let expected_availability = if toolset_ga.contains(&def.id) {
                Availability::Current
            } else {
                Availability::Legacy
            };
            assert_eq!(
                def.availability, expected_availability,
                "{} declares the wrong availability",
                def.name
            );

            let expected_tier = if high_res.contains(&def.id) {
                ImageTier::HighResolution
            } else {
                ImageTier::Standard
            };
            assert_eq!(
                def.image_tier, expected_tier,
                "{} declares the wrong image tier",
                def.name
            );
        }
    }

    /// Image tier and tool version are separate axes. Opus 4.6 shares Opus
    /// 4.7's tool version but not its image tier, which is exactly the pairing
    /// the old single list got wrong.
    #[test]
    fn image_tier_is_not_inferred_from_the_tool_version() {
        use crate::constants::ui::standard_resolutions::supports_high_res;

        assert_eq!(
            Provider::Anthropic.computer_use(model_ids::CLAUDE_OPUS_4_7),
            Provider::Anthropic.computer_use(model_ids::CLAUDE_OPUS_4_6),
            "these two models take the same computer-use tool version"
        );
        assert!(supports_high_res(model_ids::CLAUDE_OPUS_4_7));
        assert!(!supports_high_res(model_ids::CLAUDE_OPUS_4_6));
        assert!(!supports_high_res(model_ids::CLAUDE_OPUS_4_5));
        assert!(!supports_high_res(model_ids::CLAUDE_SONNET_4_6));
    }

    /// Exactly one model per provider is recommended, because `default_model`
    /// picks the first recommended one.
    #[test]
    fn each_provider_recommends_exactly_one_model() {
        for provider in [
            Provider::Anthropic,
            Provider::OpenAI,
            Provider::Rig,
            Provider::Gemini,
            Provider::ClaudeCli,
        ] {
            let recommended: Vec<_> = provider
                .model_definitions()
                .iter()
                .filter(|def| def.is_recommended)
                .collect();
            assert_eq!(
                recommended.len(),
                1,
                "{} should recommend exactly one model",
                provider.id()
            );
            // And the default must be one Juno shows without advanced settings.
            assert_eq!(recommended[0].availability, Availability::Current);
            assert_eq!(provider.default_model(), recommended[0].id);
        }
    }

    /// A capable model gets its own declared version's tool set and beta flag,
    /// not a hardcoded default.
    #[test]
    fn capable_model_yields_its_declared_version() {
        // Haiku 4.5 is a computer-use model; it just takes the earlier version.
        let haiku = model_ids::CLAUDE_HAIKU_4_5;
        assert!(Provider::Anthropic.model_supports_computer_use(haiku));
        assert_eq!(
            Provider::Anthropic.computer_use_beta_flag(haiku),
            Some("computer-use-2025-01-24")
        );
        assert_eq!(
            Provider::Anthropic.resolve_tool_type("computer", "computer_20250124", haiku),
            "computer_20250124"
        );

        // The default model is on the newer version and must not inherit the
        // old hardcoded default.
        let fable = model_ids::CLAUDE_FABLE_5_1;
        assert_eq!(
            Provider::Anthropic.computer_use_beta_flag(fable),
            Some("computer-use-2025-11-24")
        );
        assert_eq!(
            Provider::Anthropic.resolve_tool_type("computer", "computer_20250124", fable),
            "computer_20251124"
        );
    }

    /// A model Juno cannot drive the computer with carries no computer-use tool
    /// version and no beta flag. Settings keep whatever model string was saved,
    /// so a model retired out of the catalog still reaches this path.
    #[test]
    fn chat_only_model_yields_no_tools_and_no_beta_flag() {
        let retired = "claude-opus-4-1-20250805";

        assert!(!Provider::Anthropic.knows_model(retired));
        assert!(!Provider::Anthropic.model_supports_computer_use(retired));
        assert_eq!(Provider::Anthropic.computer_use_beta_flag(retired), None);
        assert_eq!(
            Provider::Anthropic
                .computer_use(retired)
                .anthropic_version(),
            None
        );
        assert_eq!(
            Provider::Anthropic.get_model_category(retired),
            ModelCategory::GeneralChat
        );
    }

    /// The refusal names the model and says what to do about it.
    #[test]
    fn refusal_names_the_model_and_the_fix() {
        let refusal = Provider::Anthropic
            .computer_use_refusal("claude-opus-4-1-20250805")
            .expect("a model outside the catalog must be refused");
        assert!(refusal.contains("claude-opus-4-1-20250805"));
        assert!(refusal.contains("Settings > AI Provider"));

        // A capable model is never refused.
        assert!(Provider::Anthropic
            .computer_use_refusal(model_ids::CLAUDE_HAIKU_4_5)
            .is_none());
    }

    /// Everything derived from the declaration must agree with it, including
    /// the shape that gets serialized to the frontend.
    #[test]
    fn derived_helpers_agree_with_the_declaration() {
        for provider in [
            Provider::Anthropic,
            Provider::OpenAI,
            Provider::Rig,
            Provider::Gemini,
            Provider::ClaudeCli,
        ] {
            for def in provider.model_definitions() {
                let capable = def.computer_use.is_supported();

                assert_eq!(def.supports_computer_use(), capable, "{}", def.name);
                assert_eq!(
                    provider.model_supports_computer_use(def.id),
                    capable,
                    "{}",
                    def.name
                );
                assert_eq!(
                    def.category() == ModelCategory::ComputerUse,
                    capable,
                    "{}",
                    def.name
                );
                assert_eq!(
                    provider.computer_use_refusal(def.id).is_none(),
                    capable,
                    "{}",
                    def.name
                );

                let info = ModelInfo::from(def);
                assert_eq!(info.supports_computer_use, capable, "{}", def.name);
                assert_eq!(info.category, def.category(), "{}", def.name);
            }
        }
    }

    /// The screenshot resolution tier reads the per-model declaration rather
    /// than keeping a second list of model IDs.
    #[test]
    fn high_res_tier_follows_the_declared_image_tier() {
        use crate::constants::ui::standard_resolutions::supports_high_res;

        assert!(supports_high_res(model_ids::CLAUDE_FABLE_5_1));
        assert!(supports_high_res(model_ids::CLAUDE_OPUS_5));
        assert!(!supports_high_res(model_ids::CLAUDE_HAIKU_4_5));
        assert!(!supports_high_res(model_ids::CLAUDE_SONNET_4_5));
        // Claude CLI aliases track the current generation.
        assert!(supports_high_res("opus"));
        assert!(supports_high_res("sonnet"));
        // An unknown model gets the safe, smaller tier.
        assert!(!supports_high_res("claude-opus-4-1-20250805"));
    }

    /// Every provider's default must be a model Juno shows without advanced
    /// settings, and every current Gemini/OpenAI entry must be a real model ID
    /// from those providers' docs.
    #[test]
    fn other_providers_offer_current_models() {
        assert_eq!(
            Provider::Gemini.default_model(),
            model_ids::GEMINI_3_8_FLASH
        );
        assert_eq!(Provider::OpenAI.default_model(), model_ids::OPENAI_SOL_5_6);
        assert_eq!(Provider::Rig.default_model(), model_ids::OPENAI_SOL_5_6);

        // The retired OpenAI model stays reachable but never by default.
        let cua = Provider::OpenAI
            .model_definitions()
            .iter()
            .find(|def| def.id == model_ids::OPENAI_CUA)
            .expect("retired model kept for saved settings");
        assert_eq!(cua.availability, Availability::Legacy);
        assert!(!cua.is_recommended);

        // Google labels the 2.5 preview legacy, so it is advanced-only.
        let gemini_25 = Provider::Gemini
            .model_definitions()
            .iter()
            .find(|def| def.id == model_ids::GEMINI_2_5_COMPUTER_USE_PREVIEW)
            .expect("legacy Gemini preview still selectable");
        assert_eq!(gemini_25.availability, Availability::Legacy);
    }

    /// Sonnet 4.5 and Haiku 4.5 are computer-use models on an older tool
    /// version. Marking them chat-only would remove working function.
    #[test]
    fn older_anthropic_models_are_legacy_not_chat_only() {
        for id in [model_ids::CLAUDE_SONNET_4_5, model_ids::CLAUDE_HAIKU_4_5] {
            assert!(
                Provider::Anthropic.model_supports_computer_use(id),
                "{id} supports computer use via computer_20250124"
            );
            assert_eq!(
                Provider::Anthropic.computer_use_beta_flag(id),
                Some("computer-use-2025-01-24")
            );
        }
    }
}
