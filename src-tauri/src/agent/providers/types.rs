//! Shared provider types — the single source of truth for provider identity,
//! model IDs, and model metadata. No dependencies on config or factory.

use crate::agent::tools::tool_versioning::ApiVersion;

// Model ID Constants - Single source of truth
pub mod model_ids {
    // Anthropic Claude Models — the current lineup, per the "Compare models"
    // table at <https://platform.claude.com/docs/en/about-claude/models/overview>.
    pub const CLAUDE_FABLE_5_1: &str = "claude-fable-5-1";
    pub const CLAUDE_OPUS_5_5: &str = "claude-opus-5-5";
    pub const CLAUDE_SONNET_5: &str = "claude-sonnet-5";
    pub const CLAUDE_HAIKU_4_5: &str = "claude-haiku-4-5-20251001";

    // Anthropic Claude Models — "Legacy models (still available)" on that same
    // page: Fable 5, Opus 5, Opus 4.8, Opus 4.7, Opus 4.6, Opus 4.5,
    // Sonnet 4.6, Sonnet 4.5. Still callable, so they stay selectable, but
    // only under advanced settings.
    //
    // NOTE: Only models the Anthropic API still serves belong here. Opus 4.1
    // (retired 2026-08-05), Opus 4, and Sonnet 4 (both retired 2026-06-15) were
    // removed — requests to retired models return 404. Verify against the live
    // deprecations page before adding an ID back:
    // https://platform.claude.com/docs/en/about-claude/model-deprecations
    pub const CLAUDE_FABLE_5: &str = "claude-fable-5";
    pub const CLAUDE_OPUS_5: &str = "claude-opus-5";
    pub const CLAUDE_OPUS_4_8: &str = "claude-opus-4-8";
    pub const CLAUDE_OPUS_4_7: &str = "claude-opus-4-7";
    pub const CLAUDE_OPUS_4_6: &str = "claude-opus-4-6";
    pub const CLAUDE_OPUS_4_5: &str = "claude-opus-4-5-20251101";
    pub const CLAUDE_SONNET_4_6: &str = "claude-sonnet-4-6";
    pub const CLAUDE_SONNET_4_5: &str = "claude-sonnet-4-5-20250929";

    // Claude Mythos 5 / 5.1 are deliberately absent: they are invite-only
    // Project Glasswing access, so listing them would offer every user a model
    // almost none of them can call.

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
    // OpenAI's `computer-use-preview` model was shut down 2026-07-23 and is
    // gone from this catalog. Do NOT reintroduce it, and do not confuse it with
    // `constants::events::tools::COMPUTER_USE_PREVIEW`, which is an unrelated
    // internal Tauri event name for the desktop cursor overlay.

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
    /// Whether *Juno* can drive the computer with this model today.
    ///
    /// There is no longer a "capable but unusable" case: Juno sends both the
    /// legacy `computer_*` tools and the `computer_toolset_20260801` toolset,
    /// so every model that can drive the computer has a tool version Juno can
    /// actually send it.
    pub fn is_supported(&self) -> bool {
        !matches!(self, ComputerUse::No)
    }

    /// Whether this model drives the computer through the
    /// `computer_toolset_20260801` toolset rather than a single `computer`
    /// tool.
    ///
    /// This is the one switch between the two wire formats. Everything the
    /// toolset changes — no `name` on the request entry, no beta header, member
    /// tool names instead of `input.action`, `toolset_name` echoed on results —
    /// hangs off this answer, and it is read from the tool version the model
    /// declares, never from `availability` or `toolset_ga`.
    pub fn uses_toolset(&self) -> bool {
        self.anthropic_version()
            .is_some_and(|version| version.is_toolset())
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
/// This tracks the provider's own lifecycle label, not any tool version.
/// For Anthropic that is the split on
/// <https://platform.claude.com/docs/en/about-claude/models/overview>: the
/// "Compare models" table is `Current` (Fable 5.1, Opus 5.5, Sonnet 5,
/// Haiku 4.5) and everything under "Legacy models (still available)" is
/// `Legacy` (Fable 5, Opus 5, Opus 4.8, Opus 4.7, Opus 4.6, Opus 4.5,
/// Sonnet 4.6, Sonnet 4.5).
///
/// `Legacy` models stay callable and stay selectable, but the picker hides
/// them unless advanced settings are on — except for the active model, which
/// is always shown so a saved choice never disappears.
///
/// Toolset GA is a *separate* axis: see `ModelDefinition::toolset_ga`. #580
/// conflated the two, which is how a model Anthropic had already moved to
/// "Legacy" ended up as Juno's recommended default.
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
    /// Whether Anthropic lists this model under `supportedModels` for the GA
    /// `computer_toolset_20260801` client toolset, at
    /// <https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool>.
    ///
    /// This is the documented fact. What Juno *sends* is the tool version in
    /// `computer_use`, and for every Anthropic row the two must agree — a model
    /// listed as toolset-GA declares `ApiVersion::ComputerToolset20260801`, and
    /// one that is not declares an earlier version. `catalog_toolset_ga_agrees_with_the_declared_tool_version`
    /// asserts exactly that, so the documented fact and the wire format cannot
    /// drift apart. False for every non-Anthropic provider.
    ///
    /// Ask `uses_computer_toolset()` when you need to know which request shape
    /// to build; this field is the citation, that method is the behaviour.
    pub toolset_ga: bool,
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

    /// Whether this model drives the computer through the
    /// `computer_toolset_20260801` toolset. Derived from the declared tool
    /// version, never from `availability` and never from `toolset_ga`.
    pub fn uses_computer_toolset(&self) -> bool {
        self.computer_use.uses_toolset()
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
    /// True when the provider lists the model as legacy. The picker hides
    /// these unless advanced settings are on, or the model is the active one.
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
            .and_then(|version| version.beta_flag())
    }

    /// Whether `model` drives the computer through the
    /// `computer_toolset_20260801` toolset rather than a single `computer` tool.
    ///
    /// The one question the provider asks to pick a wire format. An unknown
    /// model answers `false` and keeps the legacy path, which is the safe
    /// direction: the worst case is an older tool version, not a rejected
    /// request.
    pub fn uses_computer_toolset(&self, model: &str) -> bool {
        self.computer_use(model).uses_toolset()
    }

    /// The `toolset_name` that `model`'s computer tool calls carry, if it uses
    /// a toolset. `None` on the legacy path, where tool calls have no toolset.
    pub fn computer_toolset_name(&self, model: &str) -> Option<&'static str> {
        self.computer_use(model)
            .anthropic_version()
            .and_then(|version| version.toolset_name())
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
        // The Anthropic computer-use tool versions Juno sends today.
        //
        // `CU_20251124` is the DEFAULT. `CU_TOOLSET` is the current GA toolset,
        // and a model gets it only when it accepts nothing earlier — today that
        // is Opus 5.5 alone. The toolset is not an upgrade to reach for: it
        // costs roughly 2x the input-token overhead on every request (4,530 vs
        // 2,152 on an identical call, measured against the live API
        // 2026-09-22) because it ships 17 member tool definitions instead of
        // one. `toolset_ga` records which models *could* take it;
        // `catalog_toolset_ga_agrees_with_the_declared_tool_version` asserts
        // Juno never sends it to a model that is not on that list.
        const CU_TOOLSET: ComputerUse =
            ComputerUse::AnthropicTool(ApiVersion::ComputerToolset20260801);
        const CU_20251124: ComputerUse = ComputerUse::AnthropicTool(ApiVersion::Computer20251124);
        const CU_20250124: ComputerUse = ComputerUse::AnthropicTool(ApiVersion::Computer20250124);

        match self {
            Provider::Anthropic => {
                &[
                    // --- Current lineup (models overview "Compare models") ---
                    ModelDefinition {
                        id: model_ids::CLAUDE_FABLE_5_1,
                        name: "Claude Fable 5.1",
                        computer_use: CU_20251124,
                        availability: Availability::Current,
                        toolset_ga: true,
                        image_tier: ImageTier::HighResolution,
                        adaptive_thinking: true,
                        server_side_fallback: true,
                        // The recommended default. Opus 5.5 takes this over in
                        // the follow-up commit, now that Juno sends
                        // `computer_toolset_20260801`.
                        is_recommended: true,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_5_5,
                        name: "Claude Opus 5.5",
                        // Toolset-only on the Claude API: a `computer_20251124`
                        // tool returns a 400. Juno now sends the toolset, so
                        // this model drives the computer like any other.
                        computer_use: CU_TOOLSET,
                        availability: Availability::Current,
                        toolset_ga: true,
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
                        toolset_ga: true,
                        image_tier: ImageTier::HighResolution,
                        adaptive_thinking: true,
                        server_side_fallback: false,
                        is_recommended: false,
                    },
                    // Haiku 4.5 is in the current lineup even though it drives
                    // the computer through the oldest tool version Juno sends.
                    // Lifecycle and tool version are separate axes.
                    ModelDefinition {
                        id: model_ids::CLAUDE_HAIKU_4_5,
                        name: "Claude Haiku 4.5",
                        computer_use: CU_20250124,
                        availability: Availability::Current,
                        toolset_ga: false,
                        image_tier: ImageTier::Standard,
                        adaptive_thinking: false,
                        server_side_fallback: false,
                        is_recommended: false,
                    },
                    // --- Legacy models (still available) ---
                    // Anthropic's own "Legacy models (still available)" list.
                    // Every one of these still answers requests; the picker
                    // just keeps them behind advanced settings.
                    ModelDefinition {
                        id: model_ids::CLAUDE_FABLE_5,
                        name: "Claude Fable 5",
                        computer_use: CU_20251124,
                        availability: Availability::Legacy,
                        toolset_ga: true,
                        image_tier: ImageTier::HighResolution,
                        adaptive_thinking: true,
                        server_side_fallback: true,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_5,
                        name: "Claude Opus 5",
                        computer_use: CU_20251124,
                        availability: Availability::Legacy,
                        toolset_ga: true,
                        image_tier: ImageTier::HighResolution,
                        adaptive_thinking: true,
                        server_side_fallback: true,
                        is_recommended: false,
                    },
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_4_8,
                        name: "Claude Opus 4.8",
                        computer_use: CU_20251124,
                        availability: Availability::Legacy,
                        toolset_ga: true,
                        image_tier: ImageTier::HighResolution,
                        adaptive_thinking: true,
                        server_side_fallback: false,
                        is_recommended: false,
                    },
                    // Opus 4.7 is the last model on the high-resolution image
                    // tier ("Claude 4.7 and later models").
                    ModelDefinition {
                        id: model_ids::CLAUDE_OPUS_4_7,
                        name: "Claude Opus 4.7",
                        computer_use: CU_20251124,
                        availability: Availability::Legacy,
                        toolset_ga: false,
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
                        toolset_ga: false,
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
                        toolset_ga: false,
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
                        toolset_ga: false,
                        image_tier: ImageTier::Standard,
                        adaptive_thinking: false,
                        server_side_fallback: false,
                        is_recommended: false,
                    },
                    // Sonnet 4.5 drives the computer on the older
                    // computer_20250124 tool version. It is not chat only;
                    // it is just further behind.
                    ModelDefinition {
                        id: model_ids::CLAUDE_SONNET_4_5,
                        name: "Claude Sonnet 4.5",
                        computer_use: CU_20250124,
                        availability: Availability::Legacy,
                        toolset_ga: false,
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
                    toolset_ga: false,
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
                    toolset_ga: false,
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
                toolset_ga: false,
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
                    toolset_ga: false,
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
                    toolset_ga: false,
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
                    toolset_ga: false,
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
                    toolset_ga: false,
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
                    toolset_ga: false,
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
                    toolset_ga: false,
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
                    toolset_ga: false,
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
                    toolset_ga: false,
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
                    toolset_ga: false,
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
        // There is no longer a "capable but Juno cannot send it a tool it
        // accepts" case: Juno sends both the legacy computer tools and the
        // computer_toolset_20260801 toolset, so anything that reaches here is
        // genuinely chat-only or unknown.
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
                    Provider::OpenAI => model_ids::OPENAI_SOL_5_6,
                    Provider::Rig => model_ids::OPENAI_SOL_5_6,
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
        // Toolset-GA, but it also accepts the cheaper legacy tool, so that is
        // what Juno sends. Only Opus 5.5 takes the toolset.
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
        // Toolset-GA, but it also accepts the cheaper legacy tool, so that is
        // what Juno sends. Only Opus 5.5 takes the toolset.
        assert_eq!(computer, "computer_20251124");

        let editor = Provider::Anthropic.resolve_tool_type(
            "str_replace_based_edit_tool",
            "text_editor_20250429",
            model_ids::CLAUDE_OPUS_5,
        );
        assert_eq!(editor, "text_editor_20250728");
    }

    /// The bug this guards: #580 shipped `claude-opus-5` as the recommended
    /// default, and Anthropic had already moved Opus 5 to "Legacy models
    /// (still available)". No provider may ever default to a legacy model
    /// again — for any provider, not just Anthropic.
    #[test]
    fn no_provider_defaults_to_a_legacy_model() {
        for provider in [
            Provider::Anthropic,
            Provider::OpenAI,
            Provider::Rig,
            Provider::Gemini,
            Provider::ClaudeCli,
        ] {
            let default = provider
                .model_definitions()
                .iter()
                .find(|def| def.id == provider.default_model())
                .unwrap_or_else(|| panic!("{}'s default must be in its catalog", provider.id()));

            assert_eq!(
                default.availability,
                Availability::Current,
                "{} defaults to {}, which the provider lists as legacy",
                provider.id(),
                default.id
            );
        }
    }

    /// And Juno's own constraint on top of that: the Anthropic default must be
    /// a model Juno can actually drive the desktop with. Opus 5.5 is the
    /// current lineup's headline model; the follow-up commit makes it the
    /// default now that Juno sends `computer_toolset_20260801`.
    #[test]
    fn anthropic_default_can_drive_the_computer() {
        assert_eq!(
            Provider::Anthropic.default_model(),
            model_ids::CLAUDE_FABLE_5_1
        );
        assert!(
            Provider::Anthropic.model_supports_computer_use(Provider::Anthropic.default_model()),
            "a desktop-automation app must not default to a model it cannot drive the desktop with"
        );
    }

    /// Claude Opus 5.5 accepts computer use ONLY through the toolset on the
    /// Claude API — `computer_20251124` returns a 400. Now that Juno sends the
    /// toolset it is fully drivable, and it must resolve to the toolset type
    /// with no beta flag.
    #[test]
    fn opus_5_5_drives_the_computer_through_the_toolset() {
        let def = Provider::Anthropic
            .model_definitions()
            .iter()
            .find(|def| def.id == model_ids::CLAUDE_OPUS_5_5)
            .expect("Claude Opus 5.5 must be in the catalog");

        assert_eq!(def.availability, Availability::Current);
        assert!(def.toolset_ga);
        assert_eq!(
            def.computer_use,
            ComputerUse::AnthropicTool(ApiVersion::ComputerToolset20260801)
        );
        assert!(def.uses_computer_toolset());
        assert!(def.adaptive_thinking);
        assert!(def.server_side_fallback);
        assert_eq!(def.image_tier, ImageTier::HighResolution);

        // It is drivable, and the toolset is what reaches the wire.
        assert!(Provider::Anthropic.model_supports_computer_use(model_ids::CLAUDE_OPUS_5_5));
        assert!(Provider::Anthropic.uses_computer_toolset(model_ids::CLAUDE_OPUS_5_5));
        assert_eq!(
            Provider::Anthropic.computer_toolset_name(model_ids::CLAUDE_OPUS_5_5),
            Some("computer")
        );
        // GA: a beta flag here would name a tool version the request does not
        // contain.
        assert_eq!(
            Provider::Anthropic.computer_use_beta_flag(model_ids::CLAUDE_OPUS_5_5),
            None
        );
        assert_eq!(
            Provider::Anthropic.resolve_tool_type(
                "computer",
                "computer_20251124",
                model_ids::CLAUDE_OPUS_5_5
            ),
            "computer_toolset_20260801",
            "the legacy type it would otherwise send is exactly what it 400s on"
        );

        // Nothing to refuse any more.
        assert_eq!(
            Provider::Anthropic.computer_use_refusal(model_ids::CLAUDE_OPUS_5_5),
            None
        );
    }

    #[test]
    fn test_resolve_tool_type_fable_5_1_remaps() {
        let computer = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_FABLE_5_1,
        );
        // Toolset-GA, but it also accepts the cheaper legacy tool, so that is
        // what Juno sends. Only Opus 5.5 takes the toolset.
        assert_eq!(computer, "computer_20251124");
        assert_eq!(
            Provider::Anthropic.computer_use_beta_flag(model_ids::CLAUDE_FABLE_5_1),
            Some("computer-use-2025-11-24")
        );
    }

    #[test]
    fn test_adaptive_thinking_and_fallback_capabilities() {
        assert!(Provider::Anthropic.supports_adaptive_thinking(model_ids::CLAUDE_FABLE_5_1));
        assert!(Provider::Anthropic.supports_adaptive_thinking(model_ids::CLAUDE_OPUS_5_5));
        assert!(Provider::Anthropic.supports_adaptive_thinking(model_ids::CLAUDE_OPUS_5));
        assert!(!Provider::Anthropic.supports_adaptive_thinking(model_ids::CLAUDE_HAIKU_4_5));
        assert!(!Provider::Anthropic.supports_adaptive_thinking(model_ids::CLAUDE_OPUS_4_5));

        // "Claude Fable 5.1, Claude Fable 5, Claude Opus 5.5, and Claude
        // Opus 5 include safety classifiers that can decline a request."
        // <https://platform.claude.com/docs/en/build-with-claude/refusals-and-fallback>
        for id in [
            model_ids::CLAUDE_FABLE_5_1,
            model_ids::CLAUDE_FABLE_5,
            model_ids::CLAUDE_OPUS_5_5,
            model_ids::CLAUDE_OPUS_5,
        ] {
            assert!(
                Provider::Anthropic.supports_server_side_fallbacks(id),
                "{id}"
            );
        }
        for id in [model_ids::CLAUDE_SONNET_5, model_ids::CLAUDE_OPUS_4_8] {
            assert!(
                !Provider::Anthropic.supports_server_side_fallbacks(id),
                "{id}"
            );
        }
        assert!(!Provider::OpenAI.supports_adaptive_thinking(model_ids::CLAUDE_FABLE_5_1));
    }

    #[test]
    fn test_resolve_tool_type_fable_5_remaps() {
        let computer = Provider::Anthropic.resolve_tool_type(
            "computer",
            "computer_20250124",
            model_ids::CLAUDE_FABLE_5,
        );
        // Toolset-GA, but it also accepts the cheaper legacy tool, so that is
        // what Juno sends. Only Opus 5.5 takes the toolset.
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

    /// The catalog must match Anthropic's docs on all four axes. If Anthropic
    /// moves a model, this is the test that should fail.
    ///
    /// Lifecycle:    the "Compare models" table and the "Legacy models (still
    ///               available)" line at
    ///               <https://platform.claude.com/docs/en/about-claude/models/overview>
    /// Tool version: <https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool#earlier-tool-versions>
    /// Toolset GA:   the `supportedModels` list on that same page
    /// Image tier:   <https://platform.claude.com/docs/en/build-with-claude/vision#evaluate-image-size>
    #[test]
    fn anthropic_catalog_matches_the_documented_tables() {
        // "Compare models" — the whole current lineup, and all of it is in
        // Juno's catalog.
        let current_lineup = [
            model_ids::CLAUDE_FABLE_5_1,
            model_ids::CLAUDE_OPUS_5_5,
            model_ids::CLAUDE_SONNET_5,
            model_ids::CLAUDE_HAIKU_4_5,
        ];

        // The docs list exactly these two under `computer_20250124`; every
        // other model Juno offers is listed under `computer_20251124`.
        let earlier_tool = [model_ids::CLAUDE_SONNET_4_5, model_ids::CLAUDE_HAIKU_4_5];

        // The models Juno actually SENDS the toolset to: the ones that accept
        // no earlier tool type, so the toolset's ~2x input-token overhead buys
        // the only working path. Verified against the live API 2026-09-22 —
        // every other toolset-GA model below returned HTTP 200 for
        // `computer_20251124`, and only Opus 5.5 returned
        // "does not support tool types: computer_20251124".
        let toolset_only = [model_ids::CLAUDE_OPUS_5_5];

        // `supportedModels` for computer_toolset_20260801, intersected with
        // what Juno offers — the documented capability, which is broader than
        // what Juno sends. (The docs also list claude-mythos-5 and
        // claude-mythos-5-1, which are invite-only and not in Juno's catalog.)
        let toolset_ga = [
            model_ids::CLAUDE_FABLE_5_1,
            model_ids::CLAUDE_OPUS_5_5,
            model_ids::CLAUDE_SONNET_5,
            model_ids::CLAUDE_FABLE_5,
            model_ids::CLAUDE_OPUS_5,
            model_ids::CLAUDE_OPUS_4_8,
        ];

        // "High-resolution | Claude 4.7 and later models". Opus 4.7 is legacy
        // on lifecycle but high-resolution on image tier.
        let high_res = [
            model_ids::CLAUDE_FABLE_5_1,
            model_ids::CLAUDE_OPUS_5_5,
            model_ids::CLAUDE_SONNET_5,
            model_ids::CLAUDE_FABLE_5,
            model_ids::CLAUDE_OPUS_5,
            model_ids::CLAUDE_OPUS_4_8,
            model_ids::CLAUDE_OPUS_4_7,
        ];

        for id in current_lineup {
            assert!(
                Provider::Anthropic.knows_model(id),
                "{id} is in the current lineup but missing from the catalog"
            );
        }

        for def in Provider::Anthropic.model_definitions() {
            let expected_computer_use = if toolset_only.contains(&def.id) {
                ComputerUse::AnthropicTool(ApiVersion::ComputerToolset20260801)
            } else if earlier_tool.contains(&def.id) {
                ComputerUse::AnthropicTool(ApiVersion::Computer20250124)
            } else {
                ComputerUse::AnthropicTool(ApiVersion::Computer20251124)
            };
            assert_eq!(
                def.computer_use, expected_computer_use,
                "{} declares the wrong computer-use tool version",
                def.name
            );

            let expected_availability = if current_lineup.contains(&def.id) {
                Availability::Current
            } else {
                Availability::Legacy
            };
            assert_eq!(
                def.availability, expected_availability,
                "{} declares the wrong availability",
                def.name
            );

            assert_eq!(
                def.toolset_ga,
                toolset_ga.contains(&def.id),
                "{} declares the wrong computer_toolset_20260801 GA status",
                def.name
            );

            // The citation bounds the behaviour in one direction: Juno may
            // only send the toolset to a model Anthropic lists as toolset-GA.
            // The converse is deliberately NOT asserted — most toolset-GA
            // models also accept `computer_20251124`, and Juno prefers it
            // because the toolset costs roughly 2x the input-token overhead
            // (4,530 vs 2,152 on an identical request, measured 2026-09-22).
            // Only a model with no legacy option is worth that.
            if def.uses_computer_toolset() {
                assert!(
                    def.toolset_ga,
                    "{} sends the toolset but is not documented as toolset-GA",
                    def.name
                );
            }

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

    /// Lifecycle and toolset GA are separate axes, and #580 conflated them.
    /// Opus 4.8 is toolset-GA yet legacy; Haiku 4.5 is current yet not
    /// toolset-GA. Either direction must be representable.
    #[test]
    fn the_toolset_path_is_selected_per_model_and_leaves_the_rest_alone() {
        for (model, expected_type, expected_flag) in [
            (
                model_ids::CLAUDE_OPUS_5_5,
                "computer_toolset_20260801",
                None,
            ),
            // Toolset-GA, but they accept the cheaper legacy tool too, so
            // that is the path Juno picks for them.
            (
                model_ids::CLAUDE_FABLE_5_1,
                "computer_20251124",
                Some("computer-use-2025-11-24"),
            ),
            (
                model_ids::CLAUDE_SONNET_5,
                "computer_20251124",
                Some("computer-use-2025-11-24"),
            ),
            (
                model_ids::CLAUDE_FABLE_5,
                "computer_20251124",
                Some("computer-use-2025-11-24"),
            ),
            (
                model_ids::CLAUDE_OPUS_5,
                "computer_20251124",
                Some("computer-use-2025-11-24"),
            ),
            (
                model_ids::CLAUDE_OPUS_4_8,
                "computer_20251124",
                Some("computer-use-2025-11-24"),
            ),
            (
                model_ids::CLAUDE_OPUS_4_7,
                "computer_20251124",
                Some("computer-use-2025-11-24"),
            ),
            (
                model_ids::CLAUDE_OPUS_4_6,
                "computer_20251124",
                Some("computer-use-2025-11-24"),
            ),
            (
                model_ids::CLAUDE_SONNET_4_6,
                "computer_20251124",
                Some("computer-use-2025-11-24"),
            ),
            (
                model_ids::CLAUDE_OPUS_4_5,
                "computer_20251124",
                Some("computer-use-2025-11-24"),
            ),
            (
                model_ids::CLAUDE_SONNET_4_5,
                "computer_20250124",
                Some("computer-use-2025-01-24"),
            ),
            (
                model_ids::CLAUDE_HAIKU_4_5,
                "computer_20250124",
                Some("computer-use-2025-01-24"),
            ),
        ] {
            let uses_toolset = expected_type == "computer_toolset_20260801";
            assert_eq!(
                Provider::Anthropic.uses_computer_toolset(model),
                uses_toolset,
                "{model} picked the wrong path"
            );
            assert_eq!(
                Provider::Anthropic.resolve_tool_type("computer", expected_type, model),
                expected_type,
                "{model} resolved the wrong computer tool type"
            );
            assert_eq!(
                Provider::Anthropic.computer_use_beta_flag(model),
                expected_flag,
                "{model} sent the wrong beta flag"
            );
            assert_eq!(
                Provider::Anthropic.computer_toolset_name(model),
                if uses_toolset { Some("computer") } else { None },
                "{model} reported the wrong toolset name"
            );

            // The editor and bash tools are identical on both paths.
            assert_eq!(
                Provider::Anthropic.resolve_tool_type(
                    "str_replace_based_edit_tool",
                    "text_editor_20250728",
                    model
                ),
                "text_editor_20250728"
            );
            assert_eq!(
                Provider::Anthropic.resolve_tool_type("bash", "bash_20250124", model),
                "bash_20250124"
            );

            // Every Anthropic model drives the computer; none is chat-only now
            // that the toolset placeholder is gone.
            assert!(Provider::Anthropic.model_supports_computer_use(model));
        }
    }

    /// Providers that drive the desktop with Juno's own function tools have no
    /// Anthropic tool version at all, so a `Current` lifecycle can never pull
    /// them onto the toolset path.
    #[test]
    fn function_tool_providers_are_never_on_the_toolset_path() {
        for provider in [
            Provider::OpenAI,
            Provider::Gemini,
            Provider::Rig,
            Provider::ClaudeCli,
        ] {
            for def in provider.model_definitions() {
                assert!(
                    !def.uses_computer_toolset(),
                    "{} on {:?} must not claim the Anthropic toolset",
                    def.name,
                    provider
                );
                assert!(!def.toolset_ga, "{} must not record toolset GA", def.name);
                assert!(!provider.uses_computer_toolset(def.id));
                assert_eq!(provider.computer_toolset_name(def.id), None);
            }
        }
    }

    /// An unknown model falls back to the legacy path, never the toolset. Too
    /// old a tool version degrades; a toolset the model does not support is a
    /// hard rejection.
    #[test]
    fn an_unknown_model_does_not_get_the_toolset() {
        assert!(!Provider::Anthropic.uses_computer_toolset("claude-does-not-exist"));
        assert_eq!(
            Provider::Anthropic.computer_toolset_name("claude-does-not-exist"),
            None
        );
    }

    #[test]
    fn lifecycle_is_not_inferred_from_toolset_ga() {
        let by_id = |id: &str| {
            Provider::Anthropic
                .model_definitions()
                .iter()
                .find(|def| def.id == id)
                .expect("model must be in the catalog")
        };

        let opus_4_8 = by_id(model_ids::CLAUDE_OPUS_4_8);
        assert!(opus_4_8.toolset_ga);
        assert_eq!(opus_4_8.availability, Availability::Legacy);

        let haiku = by_id(model_ids::CLAUDE_HAIKU_4_5);
        assert!(!haiku.toolset_ga);
        assert_eq!(haiku.availability, Availability::Current);
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

        // OpenAI's `computer-use-preview` model shut down 2026-07-23 and must
        // not come back. A saved setting naming it still resolves — Juno keeps
        // the string and treats it as an unknown model — but the catalog does
        // not advertise a model that no longer answers.
        for provider in [Provider::OpenAI, Provider::Rig] {
            assert!(
                !provider.knows_model("computer-use-preview"),
                "{} must not list the shut-down OpenAI CUA model",
                provider.id()
            );
            assert!(
                !provider.model_definitions().is_empty(),
                "{} must not be left with an empty catalog",
                provider.id()
            );
        }

        // Google labels the 2.5 preview legacy, so it is advanced-only.
        let gemini_25 = Provider::Gemini
            .model_definitions()
            .iter()
            .find(|def| def.id == model_ids::GEMINI_2_5_COMPUTER_USE_PREVIEW)
            .expect("legacy Gemini preview still selectable");
        assert_eq!(gemini_25.availability, Availability::Legacy);
    }

    /// Sonnet 4.5 and Haiku 4.5 drive the computer through the oldest tool
    /// version Juno sends. Marking them chat-only would remove working
    /// function — and note Haiku 4.5 is in the *current* lineup, so an old
    /// tool version is not the same thing as a legacy model.
    #[test]
    fn older_tool_version_models_are_not_chat_only() {
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
