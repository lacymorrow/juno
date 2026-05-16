use computer_use_ai_sdk::Desktop;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::Weak;
use std::time::Duration;
use tauri::Emitter;
use tokio::sync::{watch, Mutex as TokioMutex};
use tracing::{debug, error, info, warn};
use crate::constants::settings::defaults;

pub mod desktop_wrapper;
use crate::commands::shell::ShellSessions;
pub use desktop_wrapper::DesktopWrapper;
use playwright::api::playwright::Playwright;

// Import the BrowserController for persistent storage
use crate::agent::tools::browser_controller::BrowserController;
// Import the memory manager for persistent conversation state
// Import permissions types
use crate::commands::permissions::PermissionsState;
// Import tool configuration manager
use crate::agent::tools::tool_config::ToolConfigManager;
// Import cloud client
use crate::cloud::{CloudClient, CloudConfig, ProductionCloudConnector};
// Import MCP manager for external MCP server support
use crate::agent::tools::mcp_integration::{MCPManager, MCPServerStatus};
// Import LocalToolProvider for tool provider registry
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::constants::{audio, events, errors::templates};
use crate::utils::string_cache::format_error_cached;
use crate::utils::rate_limiter::GlobalRateLimiters;

// Helper function for error formatting - uses cached templates for better performance
fn format_error(template: &'static str, context: &str, error: impl std::fmt::Display) -> String {
    format_error_cached(template, context, error)
}

/// Keyboard shortcut configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardShortcuts {
    pub agent_mode: String, // Default: Alt+D (Option+D on macOS)
    pub dictation_input: String,   // Default: Alt+Space (Option+Space on macOS)
    pub stop_current_task: String, // Default: Escape
    pub open_settings: String,     // Default: Cmd+, (Ctrl+, on non-macOS)
}

/// Agent trigger mode configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum AgentTriggerMode {
    #[default]
    Tap,  // Press and release to toggle agent mode
    Hold, // Hold to activate agent mode, release to stop
}

/// Dictation trigger mode configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum DictationTriggerMode {
    Tap,  // Press and release to toggle dictation mode
    #[default]
    Hold, // Hold to activate dictation mode, release to stop
}

impl Default for KeyboardShortcuts {
    fn default() -> Self {
        Self {
            agent_mode: defaults::AGENT_MODE.to_string(),
            dictation_input: defaults::DICTATION_INPUT.to_string(),
            stop_current_task: defaults::STOP_CURRENT_TASK.to_string(),
            open_settings: defaults::OPEN_SETTINGS.to_string(),
        }
    }
}

/// Timestamp tracking for log grouping (Slack/Apple Messages style)
#[derive(Debug, Clone)]
pub struct TimestampTracker {
    pub last_timestamp_shown: Option<u64>,
    pub events_since_last_timestamp: usize,
}

#[allow(clippy::new_without_default)]
impl TimestampTracker {
    pub fn new() -> Self {
        Self {
            last_timestamp_shown: None,
            events_since_last_timestamp: 0,
        }
    }

    pub fn record_event(&mut self, timestamp: u64, should_show_timestamp: bool) {
        if should_show_timestamp {
            self.last_timestamp_shown = Some(timestamp);
            self.events_since_last_timestamp = 0;
        } else {
            self.events_since_last_timestamp += 1;
        }
    }
}

// Define a type alias for the cancellation sender for clarity
type CancelSender = watch::Sender<bool>;
// Define a type alias for the cancellation receiver for clarity
pub type CancelReceiver = watch::Receiver<bool>;

/// Per-agent cursor position for the desktop overlay (Phase 4 multi-agent cursors).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCursorState {
    pub agent_id: String,
    pub x: f64,
    pub y: f64,
    /// "idle" | "moving" | "clicking" | "thinking"
    pub state: String,
    /// CSS color string for this agent's cursor sprite
    pub color: String,
}

/// Risk level for tool approval requests
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

/// Tool approval request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolApprovalRequest {
    pub tool_id: String,
    pub tool_name: String,
    pub tool_input: Value,
    pub description: String,
    pub timestamp: u64,
    pub approved: Option<bool>, // None = pending, Some(true) = approved, Some(false) = denied
    pub risk_level: RiskLevel,
    pub target_app: Option<String>,
    pub timeout_seconds: u64,
}

impl ToolApprovalRequest {
    pub fn new(tool_id: String, tool_name: String, tool_input: Value, description: String) -> Self {
        Self {
            tool_id,
            tool_name,
            tool_input,
            description,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_millis() as u64,
            approved: None,
            risk_level: RiskLevel::default(),
            target_app: None,
            timeout_seconds: 60,
        }
    }

    pub fn with_risk(mut self, risk_level: RiskLevel) -> Self {
        self.risk_level = risk_level;
        self
    }

    pub fn with_target_app(mut self, app: String) -> Self {
        self.target_app = Some(app);
        self
    }

    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }
}

// Consolidated state structures to reduce Arc<Mutex<T>> count

/// Audio-related settings grouped together
#[derive(Clone, Debug)]
pub struct AudioSettings {
    pub tts_provider: String,
    pub dictation_active: bool,
    pub dictation_clipboard_enabled: bool,
    pub sound_enabled: bool,
    pub always_listening_active: bool,
    pub always_listening_sensitivity: f32,
    pub always_listening_wake_words: Vec<String>,
    pub notification_sound_enabled: bool,
    pub was_always_listening_active_before_dictation: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            tts_provider: {
                #[cfg(debug_assertions)]
                {
                    "system".to_string()
                }
                #[cfg(not(debug_assertions))]
                {
                    "elevenlabs".to_string()
                }
            },
            dictation_active: false,
            dictation_clipboard_enabled: true,
            sound_enabled: true,
            always_listening_active: false,
            always_listening_sensitivity: 0.5,
            always_listening_wake_words: audio::DEFAULT_WAKE_WORDS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            notification_sound_enabled: true,
            was_always_listening_active_before_dictation: false,
        }
    }
}

/// Agent execution state grouped together
#[derive(Clone, Debug, Default)]
pub struct AgentExecutionState {
    pub execution_active: bool,
    pub execution_id: Option<String>,
    pub current_step: Option<u32>,
    pub max_steps: Option<u32>,
    pub tool_approval_required: bool,
}

/// UI and display settings grouped together
#[derive(Clone, Debug)]
pub struct UISettings {
    pub bar_ui_state: String,
    pub performance_monitoring_enabled: bool,
    pub debug_mode: bool,
    pub notification_type: String,
    pub notification_duration: u32,
    pub notification_position: String,
    pub notification_show_icons: bool,
    pub notification_persist_important: bool,
    pub smooth_mouse_movement: bool,
}

impl Default for UISettings {
    fn default() -> Self {
        Self {
            bar_ui_state: "default".to_string(),
            performance_monitoring_enabled: true,
            debug_mode: false,
            notification_type: "system".to_string(),
            notification_duration: 5000,
            notification_position: "bottom-right".to_string(),
            notification_show_icons: true,
            notification_persist_important: true,
            smooth_mouse_movement: true, // Default to smooth movement for better UX
        }
    }
}

/// Input configuration grouped together
#[derive(Clone, Debug, Default)]
pub struct InputSettings {
    pub keyboard_shortcuts: KeyboardShortcuts,
    pub agent_trigger_mode: AgentTriggerMode,
    pub dictation_trigger_mode: DictationTriggerMode,
}

/// Synchronized wrapper for backward compatibility
/// This provides Deref/DerefMut to the actual value while keeping state synchronized
///
/// Application state structure - Simplified with grouped settings
#[derive(Clone)] // AppState needs to be Clone
pub struct AppState {
    pub desktop: DesktopWrapper,
    pub shell_sessions: ShellSessions,
    cancel_tx: Arc<CancelSender>,  // Store Sender to signal cancellation
    pub cancel_rx: CancelReceiver, // Store Receiver to check for cancellation

    // Grouped settings structures (major simplification)
    pub audio_settings: Arc<StdMutex<AudioSettings>>,
    pub agent_execution: Arc<StdMutex<AgentExecutionState>>,
    pub ui_settings: Arc<StdMutex<UISettings>>,
    pub input_settings: Arc<StdMutex<InputSettings>>,

    // Essential state that needs separate control
    pub last_edited_file: Arc<StdMutex<Option<PathBuf>>>,
    pub previous_content: Arc<StdMutex<Option<Option<String>>>>,
    pub timestamp_tracker: Arc<StdMutex<TimestampTracker>>,

    // Async state that needs TokioMutex
    playwright_driver: Arc<TokioMutex<Option<Arc<Playwright>>>>,
    pub browser_controller: Arc<TokioMutex<Option<BrowserController>>>,
    pub memory_manager: Arc<TokioMutex<crate::agent::implementations::memory_manager::AdvancedMemoryManager>>,
    pub permissions_state: Arc<TokioMutex<Option<PermissionsState>>>,
    pub tool_config_manager: Arc<TokioMutex<ToolConfigManager>>,
    pub cloud_client: Arc<TokioMutex<Option<CloudClient>>>,
    pub cloud_config: Arc<TokioMutex<CloudConfig>>,
    pub production_cloud_connector: Arc<TokioMutex<Option<ProductionCloudConnector>>>,
    pub mcp_manager: Arc<TokioMutex<MCPManager>>,
    pub tool_provider_registry: Arc<TokioMutex<Vec<Weak<TokioMutex<LocalToolProvider>>>>>,
    pub pending_tool_approvals: Arc<TokioMutex<HashMap<String, ToolApprovalRequest>>>,

    // Pre-captured screenshot from PTT release (set during STT finalization, consumed on first agent screenshot call).
    // Tuple stores (screenshot, capture_time) so stale entries can be discarded.
    pub pending_ptt_screenshot: Arc<TokioMutex<Option<(crate::commands::core::ScreenshotResult, std::time::Instant)>>>,

    // Simple state fields
    pub permissions_checked: Arc<StdMutex<bool>>,
    pub cloud_enabled: Arc<StdMutex<bool>>,
    /// Runtime flag: true when the onboarding window is actively showing.
    /// Used to suppress agent/dictation actions while still providing visual shortcut feedback.
    onboarding_active: Arc<std::sync::atomic::AtomicBool>,

    // Rate limiting for command safety
    pub rate_limiters: Arc<GlobalRateLimiters>,

    // Per-agent cursor positions for multi-agent overlay (Phase 4)
    pub agent_cursors: Arc<StdMutex<HashMap<String, AgentCursorState>>>,

    // Dynamic storage for other state components
    state_components: Arc<StdMutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
}

impl AppState {
    pub fn new(desktop: Option<Arc<Desktop>>) -> Self {
        let (cancel_tx, cancel_rx) = watch::channel(false); // Initial state: not cancelled
        info!("Initializing AppState with simplified grouped structure");
        
        // Create rate limiters (cleanup task will be started later in async context)
        let rate_limiters = Arc::new(GlobalRateLimiters::new());
        
        Self {
            desktop: DesktopWrapper::new(desktop),
            shell_sessions: ShellSessions::default(),
            cancel_tx: Arc::new(cancel_tx),
            cancel_rx,

            // Initialize grouped settings
            audio_settings: Arc::new(StdMutex::new(AudioSettings::default())),
            agent_execution: Arc::new(StdMutex::new(AgentExecutionState::default())),
            ui_settings: Arc::new(StdMutex::new(UISettings::default())),
            input_settings: Arc::new(StdMutex::new(InputSettings::default())),

            // Initialize essential state
            last_edited_file: Arc::new(StdMutex::new(None)),
            previous_content: Arc::new(StdMutex::new(None)),
            timestamp_tracker: Arc::new(StdMutex::new(TimestampTracker::new())),

            // Initialize async state
            playwright_driver: Arc::new(TokioMutex::new(None)),
            browser_controller: Arc::new(TokioMutex::new(None)),
            memory_manager: Arc::new(TokioMutex::new({
            // Use AdvancedMemoryManager but with reduced features to prevent deadlocks
            use crate::agent::implementations::memory_manager::{AdvancedMemoryManager, MemoryConfig, VisualContextConfig};

            let memory_config = MemoryConfig {
                max_messages: 200, // INCREASED: Maximum memory capacity (was 150)
                max_tokens: 120000, // INCREASED: Higher token limit for complex conversations (was 80000)
                min_messages_to_keep: 30, // INCREASED: Keep even more context (was 20)
                auto_prune: true, // RE-ENABLED: Safe auto-pruning with higher limits
                enable_summarization: true, // RE-ENABLED: Summarization for better memory management
                summarization_batch_size: 15, // INCREASED: More efficient batching (was 12)
                enable_metrics: true, // ENABLED: Enhanced tracking with error handling
                enable_summary_cache: true, // RE-ENABLED: Cache for better performance
            };

            let visual_config = VisualContextConfig {
                enable_screenshot_compression: true, // RE-ENABLED: Visual compression for memory efficiency
                screenshot_retention_seconds: 1800, // INCREASED: Even longer retention (30 minutes, was 15)
                immediate_compression: true, // RE-ENABLED: With safer async handling
                max_base64_screenshots: 12, // INCREASED: More visual context (was 8)
                fallback_to_generic_description: true,
            };

            AdvancedMemoryManager::with_config(memory_config).with_visual_config(visual_config)
        })),
            permissions_state: Arc::new(TokioMutex::new(None)),
            tool_config_manager: Arc::new(TokioMutex::new(ToolConfigManager::new())),
            cloud_client: Arc::new(TokioMutex::new(None)),
            cloud_config: Arc::new(TokioMutex::new(CloudConfig::default())),
            production_cloud_connector: Arc::new(TokioMutex::new(None)),
            mcp_manager: Arc::new(TokioMutex::new(MCPManager::new())),
            tool_provider_registry: Arc::new(TokioMutex::new(Vec::new())),
            pending_tool_approvals: Arc::new(TokioMutex::new(HashMap::new())),
            pending_ptt_screenshot: Arc::new(TokioMutex::new(None)),

            // Initialize simple state
            permissions_checked: Arc::new(StdMutex::new(false)),
            cloud_enabled: Arc::new(StdMutex::new(false)),
            onboarding_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),

            // Use the rate limiters created above
            rate_limiters,

            // Initialize per-agent cursor tracking
            agent_cursors: Arc::new(StdMutex::new(HashMap::new())),

            // Initialize dynamic storage
            state_components: Arc::new(StdMutex::new(HashMap::new())),
        }
    }

    /// Initialize rate limiter cleanup task (must be called after Tokio runtime is ready)
    pub async fn initialize_rate_limiter_cleanup(&self) {
        info!("Starting rate limiter cleanup task");
        self.rate_limiters.clone().start_cleanup_task();
    }

    /// Maximum age for a cached PTT screenshot before it is considered stale.
    const PTT_SCREENSHOT_TTL_SECS: u64 = 10;

    /// Store a screenshot captured concurrently with PTT STT finalization.
    /// Consumed on the agent's first `computer/screenshot` tool call.
    pub async fn set_pending_ptt_screenshot(&self, screenshot: crate::commands::core::ScreenshotResult) {
        let mut guard = self.pending_ptt_screenshot.lock().await;
        *guard = Some((screenshot, std::time::Instant::now()));
    }

    /// Take (and clear) the pre-captured PTT screenshot if one exists and is not stale.
    /// Discards (and returns `None`) if the screenshot is older than `PTT_SCREENSHOT_TTL_SECS`.
    pub async fn take_pending_ptt_screenshot(&self) -> Option<crate::commands::core::ScreenshotResult> {
        let mut guard = self.pending_ptt_screenshot.lock().await;
        match guard.take() {
            Some((screenshot, captured_at))
                if captured_at.elapsed().as_secs() < Self::PTT_SCREENSHOT_TTL_SECS =>
            {
                Some(screenshot)
            }
            Some(_) => {
                warn!("[PTT] Discarding stale pre-captured screenshot (older than {}s)", Self::PTT_SCREENSHOT_TTL_SECS);
                None
            }
            None => None,
        }
    }

    // Audio Settings - Getter/Setter methods that operate on actual shared state
    pub fn get_tts_provider(&self) -> Result<String, String> {
        self.audio_settings
            .lock()
            .map(|settings| settings.tts_provider.clone())
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "TTS provider", e))
    }

    pub fn set_tts_provider(&self, provider: String) -> Result<(), String> {
        self.audio_settings
            .lock()
            .map(|mut settings| settings.tts_provider = provider)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "TTS provider", e))
    }

    pub fn get_dictation_active(&self) -> Result<bool, String> {
        self.audio_settings
            .lock()
            .map(|settings| settings.dictation_active)
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "dictation active status", e))
    }

    pub fn set_dictation_active(&self, active: bool) -> Result<(), String> {
        self.audio_settings
            .lock()
            .map(|mut settings| settings.dictation_active = active)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "dictation active status", e))
    }

    pub fn get_dictation_clipboard_enabled(&self) -> Result<bool, String> {
        self.audio_settings
            .lock()
            .map(|settings| settings.dictation_clipboard_enabled)
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "dictation clipboard enabled", e))
    }

    pub fn set_dictation_clipboard_enabled(&self, enabled: bool) -> Result<(), String> {
        self.audio_settings
            .lock()
            .map(|mut settings| settings.dictation_clipboard_enabled = enabled)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "dictation clipboard enabled", e))
    }

    pub fn get_sound_enabled(&self) -> Result<bool, String> {
        self.audio_settings
            .lock()
            .map(|settings| settings.sound_enabled)
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "sound enabled", e))
    }

    pub fn set_sound_enabled(&self, enabled: bool) -> Result<(), String> {
        self.audio_settings
            .lock()
            .map(|mut settings| settings.sound_enabled = enabled)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "sound enabled", e))
    }

    pub fn get_always_listening_active(&self) -> Result<bool, String> {
        self.audio_settings
            .lock()
            .map(|settings| settings.always_listening_active)
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "always listening active", e))
    }

    pub fn set_always_listening_active(&self, active: bool) -> Result<(), String> {
        self.audio_settings
            .lock()
            .map(|mut settings| settings.always_listening_active = active)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "always listening active", e))
    }

    pub fn get_always_listening_sensitivity(&self) -> Result<f32, String> {
        self.audio_settings
            .lock()
            .map(|settings| settings.always_listening_sensitivity)
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "always listening sensitivity", e))
    }

    pub fn set_always_listening_sensitivity(&self, sensitivity: f32) -> Result<(), String> {
        self.audio_settings
            .lock()
            .map(|mut settings| settings.always_listening_sensitivity = sensitivity)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "always listening sensitivity", e))
    }

    pub fn get_always_listening_wake_words(&self) -> Result<Vec<String>, String> {
        self.audio_settings
            .lock()
            .map(|settings| settings.always_listening_wake_words.clone())
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "always listening wake words", e))
    }

    pub fn set_always_listening_wake_words(&self, wake_words: Vec<String>) -> Result<(), String> {
        self.audio_settings
            .lock()
            .map(|mut settings| settings.always_listening_wake_words = wake_words)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "always listening wake words", e))
    }

    pub fn get_notification_sound_enabled(&self) -> Result<bool, String> {
        self.audio_settings
            .lock()
            .map(|settings| settings.notification_sound_enabled)
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "notification sound enabled", e))
    }

    pub fn set_notification_sound_enabled(&self, enabled: bool) -> Result<(), String> {
        self.audio_settings
            .lock()
            .map(|mut settings| settings.notification_sound_enabled = enabled)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "notification sound enabled", e))
    }

    // UI Settings - Getter/Setter methods
    pub fn get_bar_ui_state(&self) -> Result<String, String> {
        self.ui_settings
            .lock()
            .map(|settings| settings.bar_ui_state.clone())
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "bar UI state", e))
    }

    pub fn set_bar_ui_state(&self, state: String) -> Result<(), String> {
        self.ui_settings
            .lock()
            .map(|mut settings| settings.bar_ui_state = state)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "bar UI state", e))
    }

    pub fn get_performance_monitoring_enabled(&self) -> Result<bool, String> {
        self.ui_settings
            .lock()
            .map(|settings| settings.performance_monitoring_enabled)
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "performance monitoring enabled", e))
    }

    pub fn set_performance_monitoring_enabled_internal(&self, enabled: bool) -> Result<(), String> {
        self.ui_settings
            .lock()
            .map(|mut settings| settings.performance_monitoring_enabled = enabled)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "performance monitoring enabled", e))
    }

    pub fn get_debug_mode(&self) -> Result<bool, String> {
        self.ui_settings
            .lock()
            .map(|settings| settings.debug_mode)
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "debug mode", e))
    }

    pub fn set_debug_mode_internal(&self, enabled: bool) -> Result<(), String> {
        self.ui_settings
            .lock()
            .map(|mut settings| settings.debug_mode = enabled)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "debug mode", e))
    }

    pub fn get_notification_type(&self) -> Result<String, String> {
        self.ui_settings
            .lock()
            .map(|settings| settings.notification_type.clone())
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "notification type", e))
    }

    pub fn set_notification_type(&self, notification_type: String) -> Result<(), String> {
        self.ui_settings
            .lock()
            .map(|mut settings| settings.notification_type = notification_type)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "notification type", e))
    }

    pub fn get_notification_duration(&self) -> Result<u32, String> {
        self.ui_settings
            .lock()
            .map(|settings| settings.notification_duration)
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "notification duration", e))
    }

    pub fn set_notification_duration(&self, duration: u32) -> Result<(), String> {
        self.ui_settings
            .lock()
            .map(|mut settings| settings.notification_duration = duration)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "notification duration", e))
    }

    pub fn get_notification_position(&self) -> Result<String, String> {
        self.ui_settings
            .lock()
            .map(|settings| settings.notification_position.clone())
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "notification position", e))
    }

    pub fn set_notification_position(&self, position: String) -> Result<(), String> {
        self.ui_settings
            .lock()
            .map(|mut settings| settings.notification_position = position)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "notification position", e))
    }

    pub fn get_notification_show_icons(&self) -> Result<bool, String> {
        self.ui_settings
            .lock()
            .map(|settings| settings.notification_show_icons)
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "notification show icons", e))
    }

    pub fn set_notification_show_icons(&self, show_icons: bool) -> Result<(), String> {
        self.ui_settings
            .lock()
            .map(|mut settings| settings.notification_show_icons = show_icons)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "notification show icons", e))
    }

    pub fn get_notification_persist_important(&self) -> Result<bool, String> {
        self.ui_settings
            .lock()
            .map(|settings| settings.notification_persist_important)
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "notification persist important", e))
    }

    pub fn set_notification_persist_important(&self, persist: bool) -> Result<(), String> {
        self.ui_settings
            .lock()
            .map(|mut settings| settings.notification_persist_important = persist)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "notification persist important", e))
    }

    pub fn get_smooth_mouse_movement(&self) -> Result<bool, String> {
        self.ui_settings
            .lock()
            .map(|settings| settings.smooth_mouse_movement)
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "smooth mouse movement", e))
    }

    pub fn set_smooth_mouse_movement(&self, enabled: bool) -> Result<(), String> {
        self.ui_settings
            .lock()
            .map(|mut settings| settings.smooth_mouse_movement = enabled)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "smooth mouse movement", e))
    }

    // Input Settings - Getter/Setter methods
    pub fn get_keyboard_shortcuts(&self) -> Result<KeyboardShortcuts, String> {
        self.input_settings
            .lock()
            .map(|settings| settings.keyboard_shortcuts.clone())
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "keyboard shortcuts", e))
    }

    pub fn set_keyboard_shortcuts(&self, shortcuts: KeyboardShortcuts) -> Result<(), String> {
        self.input_settings
            .lock()
            .map(|mut settings| settings.keyboard_shortcuts = shortcuts)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "keyboard shortcuts", e))
    }

    pub fn get_agent_trigger_mode(&self) -> Result<AgentTriggerMode, String> {
        self.input_settings
            .lock()
            .map(|settings| settings.agent_trigger_mode.clone())
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "agent trigger mode", e))
    }

    pub fn set_agent_trigger_mode(&self, mode: AgentTriggerMode) -> Result<(), String> {
        self.input_settings
            .lock()
            .map(|mut settings| settings.agent_trigger_mode = mode)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "agent trigger mode", e))
    }

    pub fn get_dictation_trigger_mode(&self) -> Result<DictationTriggerMode, String> {
        self.input_settings
            .lock()
            .map(|settings| settings.dictation_trigger_mode.clone())
            .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "dictation trigger mode", e))
    }

    pub fn set_dictation_trigger_mode(&self, mode: DictationTriggerMode) -> Result<(), String> {
        self.input_settings
            .lock()
            .map(|mut settings| settings.dictation_trigger_mode = mode)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "dictation trigger mode", e))
    }

    // Method to trigger cancellation
    pub fn signal_cancel(&self) {
        // Send `true` to indicate cancellation is requested.
        // Ignore result as we don't care if there are no active receivers.
        let _ = self.cancel_tx.send(true);
    }

    // Method to reset the cancellation signal
    pub fn reset_cancel(&self) {
        // Send `false` to indicate cancellation is no longer requested.
        // Check if the current value is true before sending false to avoid unnecessary updates.
        let is_currently_cancelled = *self.cancel_rx.borrow();
        info!(
            "[AppState] Attempting reset_cancel. Current state (is_cancelled): {}",
            is_currently_cancelled
        );
        if is_currently_cancelled {
            let send_result = self.cancel_tx.send(false);
            info!(
                "[AppState] reset_cancel: Sent 'false'. Result: {:?}",
                send_result.is_ok()
            );
        } else {
            info!("[AppState] reset_cancel: No reset needed (already false).");
        }
    }

    // Method to mark agent execution started
    pub fn mark_agent_execution_started(&self, execution_id: String) -> Result<(), String> {
        let mut execution_state = self
            .agent_execution
            .lock()
            .map_err(|e| format_error(templates::FAILED_TO_ACCESS, "agent_execution lock", e))?;
        execution_state.execution_active = true;
        execution_state.execution_id = Some(execution_id.clone());
        info!(
            "[AppState] Agent execution started with ID: {}",
            execution_id
        );
        Ok(())
    }

    // Method to mark agent execution started with iteration info
    pub fn mark_agent_execution_started_with_steps(
        &self,
        execution_id: String,
        max_steps: u32,
    ) -> Result<(), String> {
        let mut execution_state = self
            .agent_execution
            .lock()
            .map_err(|e| format_error(templates::FAILED_TO_ACCESS, "agent_execution lock", e))?;
        execution_state.execution_active = true;
        execution_state.execution_id = Some(execution_id.clone());
        execution_state.max_steps = Some(max_steps);
        execution_state.current_step = Some(0); // Start at step 0
        info!(
            "[AppState] Agent execution started with ID: {} (max steps: {})",
            execution_id, max_steps
        );
        Ok(())
    }

    // Method to mark agent execution as finished
    pub fn mark_agent_execution_finished(&self) {
        let result = (|| -> Result<(), String> {
            let mut execution_state = self
                .agent_execution
                .lock()
                .map_err(|e| format_error(templates::FAILED_TO_ACCESS, "agent_execution lock", e))?;
            let execution_id = execution_state.execution_id.take();
            execution_state.execution_active = false;
            execution_state.current_step = None;
            execution_state.max_steps = None;
            info!(
                "[AppState] Agent execution finished for ID: {:?}",
                execution_id
            );
            Ok(())
        })();

        if let Err(e) = result {
            error!("Error marking agent execution as finished: {}", e);
        }
    }

    // Method to check if an agent is currently executing
    pub fn is_agent_executing(&self) -> bool {
        self.agent_execution
            .lock()
            .map(|guard| guard.execution_active)
            .unwrap_or_else(|e| {
                error!("Failed to check agent execution status: {}", e);
                false // Safe fallback
            })
    }

    // Method to get the current agent execution ID
    pub fn get_current_agent_execution_id(&self) -> Option<String> {
        self.agent_execution
            .lock()
            .map(|guard| guard.execution_id.clone())
            .unwrap_or_else(|e| {
                error!("Failed to get current agent execution ID: {}", e);
                None // Safe fallback
            })
    }

    // Method to update the current agent step
    pub fn update_agent_current_step(&self, step: u32) -> Result<(), String> {
        let mut execution_state = self
            .agent_execution
            .lock()
            .map_err(|e| format_error(templates::FAILED_TO_ACCESS, "agent_execution lock", e))?;
        execution_state.current_step = Some(step);
        debug!("[AppState] Agent current step updated to: {}", step);
        Ok(())
    }

    // Method to get the current agent step
    pub fn get_agent_current_step(&self) -> Option<u32> {
        self.agent_execution
            .lock()
            .map(|guard| guard.current_step)
            .unwrap_or_else(|e| {
                error!("Failed to get current agent step: {}", e);
                None // Safe fallback
            })
    }

    // Method to get the agent max steps
    pub fn get_agent_max_steps(&self) -> Option<u32> {
        self.agent_execution
            .lock()
            .map(|guard| guard.max_steps)
            .unwrap_or_else(|e| {
                error!("Failed to get agent max steps: {}", e);
                None // Safe fallback
            })
    }

    // Method to get agent step progress info (single lock acquisition for consistency)
    pub fn get_agent_step_progress(&self) -> (Option<u32>, Option<u32>) {
        self.agent_execution
            .lock()
            .map(|guard| (guard.current_step, guard.max_steps))
            .unwrap_or_else(|e| {
                error!("Failed to get agent step progress: {}", e);
                (None, None)
            })
    }

    /// Check if agent mode is currently active
    pub fn is_agent_mode_active(&self) -> bool {
        self.agent_execution
            .lock()
            .map(|guard| guard.execution_active)
            .unwrap_or_else(|e| {
                error!("Failed to check agent mode status: {}", e);
                false // Safe fallback
            })
    }

    /// Check if dictation mode is currently active
    pub fn is_dictation_active(&self) -> bool {
        self.audio_settings
            .lock()
            .map(|settings| settings.dictation_active)
            .unwrap_or_else(|e| {
                error!("Failed to check dictation status: {}", e);
                false // Safe fallback
            })
    }

    /// Check if agent is currently active (alias for is_agent_mode_active)
    pub fn is_agent_active(&self) -> bool {
        self.is_agent_mode_active()
    }

    /// Check if onboarding is currently active
    pub fn is_onboarding_active(&self) -> bool {
        self.onboarding_active.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Set onboarding active state
    pub fn set_onboarding_active(&self, active: bool) {
        self.onboarding_active.store(active, std::sync::atomic::Ordering::Release);
        info!("Onboarding active state set to: {}", active);
    }

    // Method to get or initialize the Playwright driver
    async fn get_or_init_playwright_driver(&self) -> Result<Arc<Playwright>, String> {
        let mut driver_guard = self.playwright_driver.lock().await;
        if driver_guard.is_none() {
            info!("Initializing Playwright driver instance...");
            match Playwright::initialize().await {
                Ok(pw_instance) => {
                    let arc_pw: Arc<Playwright> = Arc::new(pw_instance);
                    *driver_guard = Some(arc_pw.clone());
                    info!("Playwright driver initialized and stored in AppState.");
                    Ok(arc_pw)
                }
                Err(e) => {
                    let err_msg = format_error(templates::FAILED_TO_INITIALIZE, "Playwright driver", e);
                    error!("{}", err_msg);
                    Err(err_msg)
                }
            }
        } else {
            debug!("Reusing existing Playwright driver instance from AppState.");
            driver_guard
                .as_ref()
                .ok_or_else(|| "Playwright driver is None despite check".to_string())
                .cloned()
        }
    }

    // Method to get or initialize the browser controller
    // NOTE: Initializes playwright driver BEFORE acquiring browser_controller lock
    // to prevent deadlock from nested async lock acquisition.
    pub async fn get_or_init_browser_controller(&self) -> Result<BrowserController, String> {
        // First, check if controller already exists (short lock)
        {
            let controller_guard = self.browser_controller.lock().await;
            if let Some(controller) = controller_guard.as_ref() {
                debug!("Reusing existing browser controller from AppState.");
                return Ok(controller.clone());
            }
        }
        // Lock released here

        // Initialize playwright driver OUTSIDE the browser_controller lock
        info!("Initializing persistent browser controller (was None in AppState)");
        let playwright_arc = self.get_or_init_playwright_driver().await.map_err(|e| {
            format!(
                "Cannot init BrowserController without Playwright driver: {}",
                e
            )
        })?;

        let new_controller = BrowserController::new(playwright_arc).await.map_err(|e| {
            let err_msg = format_error(templates::FAILED_TO_INITIALIZE, "browser controller", e);
            error!("{}", err_msg);
            err_msg
        })?;

        // Re-acquire lock to store the controller (double-check pattern)
        let mut controller_guard = self.browser_controller.lock().await;
        if let Some(controller) = controller_guard.as_ref() {
            // Another task initialized it while we were working
            debug!("Browser controller was initialized by another task, reusing.");
            return Ok(controller.clone());
        }

        *controller_guard = Some(new_controller.clone());
        info!("BrowserController initialized and stored in AppState.");
        Ok(new_controller)
    }

    // Method to get the persistent memory manager
    pub async fn get_memory_manager(&self) -> Arc<TokioMutex<crate::agent::implementations::memory_manager::AdvancedMemoryManager>> {
        self.memory_manager.clone()
    }

    // Insert a component into the state
    pub fn insert<T: 'static + Send + Sync>(&self, component: T) -> Result<(), String> {
        let type_id = TypeId::of::<T>();
        let mut components_lock = self
            .state_components
            .lock()
            .map_err(|e| format_error(templates::FAILED_TO_ACCESS, "state_components lock", e))?;
        components_lock.insert(type_id, Box::new(component));
        Ok(())
    }

    // Get a reference to a component from the state
    pub fn get<T: 'static + Send + Sync + Clone>(&self) -> Option<Arc<T>> {
        let type_id = TypeId::of::<T>();
        let components_lock = self
            .state_components
            .lock()
            .map_err(|e| {
                error!("Failed to acquire state_components lock: {}", e);
            })
            .ok()?;

        components_lock.get(&type_id).and_then(|boxed| {
            boxed.downcast_ref::<T>().map(|value| {
                // Clone the Arc to extend the lifetime beyond the lock
                Arc::new(value.clone())
            })
        })
    }

    // Method to update permissions state
    pub async fn update_permissions_state(&self, state: PermissionsState) {
        let mut permissions_guard = self.permissions_state.lock().await;
        *permissions_guard = Some(state);
    }

    // Method to get permissions state
    pub async fn get_permissions_state(&self) -> Option<PermissionsState> {
        let permissions_guard = self.permissions_state.lock().await;
        permissions_guard.clone()
    }

    // Method to mark permissions as checked
    pub fn mark_permissions_checked(&self) -> Result<(), String> {
        let mut checked_guard = self
            .permissions_checked
            .lock()
            .map_err(|e| format_error(templates::FAILED_TO_ACCESS, "permissions_checked lock", e))?;
        *checked_guard = true;
        Ok(())
    }

    // Method to check if permissions have been checked
    pub fn are_permissions_checked(&self) -> bool {
        self.permissions_checked
            .lock()
            .map(|guard| *guard)
            .unwrap_or_else(|e| {
                error!("Failed to check permissions status: {}", e);
                false // Safe fallback
            })
    }

    // Helper method to get desktop instance or return an error
    pub fn get_desktop(&self) -> Result<&Arc<Desktop>, String> {
        self.desktop.get_desktop()
    }

    // Helper method to check if desktop automation is available
    pub fn is_desktop_available(&self) -> bool {
        self.desktop.is_available()
    }

    // Helper method to get desktop instance for situations where we can handle the error gracefully
    pub fn try_get_desktop(&self) -> Option<&Arc<Desktop>> {
        self.desktop.try_get_desktop()
    }

    // Method to get tool configuration manager
    pub async fn get_tool_config_manager(&self) -> Arc<TokioMutex<ToolConfigManager>> {
        self.tool_config_manager.clone()
    }

    // Method to load tool configuration from centralized settings
    pub async fn load_tool_config(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())
            .map_err(|e| format_error(templates::FAILED_TO_CREATE, "settings manager", e))?;
        crate::agent::tools::tool_config::load_tool_config_from_centralized_settings(
            &settings_manager,
            self,
        )
        .await
    }

    // Method to save tool configuration to centralized settings
    pub async fn save_tool_config(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())
            .map_err(|e| format_error(templates::FAILED_TO_CREATE, "settings manager", e))?;
        crate::agent::tools::tool_config::save_tool_config_to_centralized_settings(
            &settings_manager,
            self,
        )
        .await
    }

    // Cloud connectivity methods

    /// Initialize cloud client
    pub async fn init_cloud_client(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        // Load cloud configuration
        let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())
            .map_err(|e| format_error(templates::FAILED_TO_CREATE, "settings manager", e))?;
        let config = CloudConfig::load_from_centralized_settings(&settings_manager)
            .await
            .map_err(|e| format_error(templates::FAILED_TO_LOAD, "cloud config", e))?;

        // Update stored config
        {
            let mut config_guard = self.cloud_config.lock().await;
            *config_guard = config.clone();
        }

        // Update enabled status
        {
            match self.cloud_enabled.lock() {
                Ok(mut enabled) => {
                    *enabled = config.enabled;
                }
                Err(e) => {
                    error!("Failed to update cloud enabled status: {}", e);
                }
            }
        }

        // Create cloud client if enabled
        if config.enabled {
            let client = CloudClient::new(app_handle.clone())
                .await
                .map_err(|e| format_error(templates::FAILED_TO_CREATE, "cloud client", e))?;

            let mut client_guard = self.cloud_client.lock().await;
            *client_guard = Some(client);
        }

        Ok(())
    }

    /// Start cloud connectivity
    pub async fn start_cloud_client(&self) -> Result<(), String> {
        let mut client_guard = self.cloud_client.lock().await;
        if let Some(client) = client_guard.as_mut() {
            client
                .start()
                .await
                .map_err(|e| format_error(templates::FAILED_TO_START, "cloud client", e))?;
        }
        Ok(())
    }

    /// Stop cloud connectivity
    pub async fn stop_cloud_client(&self) {
        let mut client_guard = self.cloud_client.lock().await;
        *client_guard = None;
    }

    /// Check if cloud is enabled
    pub fn is_cloud_enabled(&self) -> bool {
        self.cloud_enabled
            .lock()
            .map(|enabled| *enabled)
            .unwrap_or_else(|e| {
                error!("Failed to check cloud enabled status: {}", e);
                false // Safe fallback
            })
    }

    /// Get cloud configuration
    pub async fn get_cloud_config(&self) -> CloudConfig {
        let config_guard = self.cloud_config.lock().await;
        config_guard.clone()
    }

    /// Update cloud configuration
    pub async fn update_cloud_config(
        &self,
        config: CloudConfig,
        app_handle: &tauri::AppHandle,
    ) -> Result<(), String> {
        // Save to centralized settings
        let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())
            .map_err(|e| format_error(templates::FAILED_TO_CREATE, "settings manager", e))?;
        config
            .save_to_centralized_settings(&settings_manager)
            .await
            .map_err(|e| format_error(templates::FAILED_TO_SAVE, "cloud config", e))?;

        // Update stored config
        {
            let mut config_guard = self.cloud_config.lock().await;
            *config_guard = config.clone();
        }

        // Update enabled status
        {
            match self.cloud_enabled.lock() {
                Ok(mut enabled) => {
                    *enabled = config.enabled;
                }
                Err(e) => {
                    error!("Failed to update cloud enabled status: {}", e);
                }
            }
        }

        // Restart cloud client if needed
        if config.enabled {
            self.stop_cloud_client().await;
            self.init_cloud_client(app_handle).await?;
            self.start_cloud_client().await?;
        } else {
            self.stop_cloud_client().await;
        }

        Ok(())
    }

    // Performance monitoring methods

    /// Check if performance monitoring is enabled
    pub fn is_performance_monitoring_enabled(&self) -> bool {
        self.ui_settings
            .lock()
            .map(|settings| settings.performance_monitoring_enabled)
            .unwrap_or_else(|e| {
                error!("Failed to check performance monitoring status: {}", e);
                false // Safe fallback
            })
    }

    /// Set performance monitoring enabled state
    pub fn set_performance_monitoring_enabled(&self, enabled: bool) -> Result<(), String> {
        self.ui_settings
            .lock()
            .map(|mut settings| {
                settings.performance_monitoring_enabled = enabled;
                info!(
                    "Performance monitoring {}",
                    if enabled { "enabled" } else { "disabled" }
                );
            })
            .map_err(|e| format_error(templates::FAILED_TO_SET, "performance monitoring enabled", e))
    }

    // Production cloud connector methods

    /// Set production cloud connector
    pub async fn set_production_cloud_connector(&self, connector: ProductionCloudConnector) {
        let mut connector_guard = self.production_cloud_connector.lock().await;
        *connector_guard = Some(connector);
    }

    /// Get production cloud connector
    pub fn get_production_cloud_connector(&self) -> Option<ProductionCloudConnector> {
        // FIXED: Use try_lock but with proper error handling
        // This maintains the non-blocking behavior while providing better error information
        match self.production_cloud_connector.try_lock() {
            Ok(connector_guard) => connector_guard.clone(),
            Err(_) => {
                // Lock is busy - log this for debugging but don't error
                // This is expected behavior when another operation is in progress
                debug!("Production cloud connector is busy, returning None (non-blocking call)");
                None
            }
        }
    }

    /// Get production cloud connector (async version) - RECOMMENDED
    pub async fn get_production_cloud_connector_async(&self) -> Option<ProductionCloudConnector> {
        let connector_guard = self.production_cloud_connector.lock().await;
        connector_guard.clone()
    }

    /// Clear production cloud connector
    pub async fn clear_production_cloud_connector(&self) {
        let mut connector_guard = self.production_cloud_connector.lock().await;
        *connector_guard = None;
    }

    /// Check if production cloud connector is available
    pub async fn has_production_cloud_connector(&self) -> bool {
        let connector_guard = self.production_cloud_connector.lock().await;
        connector_guard.is_some()
    }

    // MCP Manager Methods

    /// Get the MCP manager
    pub async fn get_mcp_manager(&self) -> Arc<TokioMutex<MCPManager>> {
        self.mcp_manager.clone()
    }

    /// Initialize enabled MCP servers - OPTIMIZED for parallel startup
    pub async fn initialize_mcp_servers(&self, app_handle: Option<&tauri::AppHandle>) -> Result<(), String> {
        debug!("Starting MCP server initialization...");

        // CRITICAL FIX: Load MCP server configurations from tool config manager
        // and populate the MCP manager before trying to get enabled servers
        let tool_config_manager = self.get_tool_config_manager().await;
        let config_guard = tool_config_manager.lock().await;
        let all_mcp_configs = config_guard.get_mcp_servers(); // Get all MCP server configs
        drop(config_guard);

        // Add configurations to MCP manager if not already present
        let manager = self.get_mcp_manager().await;
        let manager_guard = manager.lock().await;

        for config in &all_mcp_configs {
            // Only add if not already present to avoid duplicates
            let existing_configs = manager_guard.get_server_configs().await;
            let already_exists = existing_configs.iter().any(|existing| existing.id == config.id);

            if !already_exists {
                let mut init_config = config.clone();
                init_config.auto_start = false;
                if let Err(e) = manager_guard.add_server(init_config).await {
                    warn!("Failed to add MCP server '{}': {}", config.name, e);
                    continue;
                }
            }
        }

        // Now get enabled server configurations (filter enabled ones manually)
        let all_server_configs = manager_guard.get_server_configs().await;
        let enabled_servers: Vec<_> = all_server_configs
            .into_iter()
            .filter(|config| config.enabled)
            .collect();
        drop(manager_guard);

        if enabled_servers.is_empty() {
            debug!("No enabled MCP servers to start");
            return Ok(());
        }

        info!("Starting {} MCP server(s)...", enabled_servers.len());

        let startup_tasks: Vec<_> = enabled_servers
            .into_iter()
            .enumerate()
            .map(|(index, config)| {
                let manager = manager.clone();
                let server_name = config.name.clone();
                let server_id = config.id.clone();

                tauri::async_runtime::spawn(async move {
                    if index > 0 {
                        let stagger_delay = std::cmp::min(index * 50, 500);
                        tokio::time::sleep(Duration::from_millis(stagger_delay as u64)).await;
                    }

                    debug!("Starting MCP server '{}' (slot {})", server_name, index);

                    let manager_guard = manager.lock().await;
                    let start_result = tokio::time::timeout(
                        Duration::from_secs(
                            crate::constants::timeouts::MCP_SERVER_STARTUP_TIMEOUT_SECONDS,
                        ),
                        manager_guard.start_server(&server_id),
                    )
                    .await;
                    drop(manager_guard);

                    match start_result {
                        Ok(Ok(_)) => {
                            debug!("MCP server '{}' started", server_name);

                            let readiness_start = std::time::Instant::now();
                            let mut check_delay = 10u64;
                            let max_delay = 200u64;
                            let max_checks = 20;

                            for check_attempt in 0..max_checks {
                                tokio::time::sleep(Duration::from_millis(check_delay)).await;

                                let manager_guard = manager.lock().await;
                                let server_statuses = manager_guard.get_server_statuses().await;
                                let server_status = server_statuses.get(&server_id);
                                let available_tools = manager_guard.get_all_tools().await;
                                let server_tools_count = available_tools.iter()
                                    .filter(|tool| tool.server_id == server_id)
                                    .count();
                                drop(manager_guard);

                                match server_status {
                                    Some(crate::agent::tools::mcp_integration::MCPServerStatus::Connected) => {
                                        debug!("MCP server '{}' ready ({}ms, {} tools)",
                                              server_name, readiness_start.elapsed().as_millis(), server_tools_count);
                                        break;
                                    }
                                    Some(crate::agent::tools::mcp_integration::MCPServerStatus::Error(ref error)) => {
                                        warn!("MCP server '{}' failed: {}", server_name, error);
                                        break;
                                    }
                                    _ => {
                                        check_delay = std::cmp::min(check_delay * 2, max_delay);
                                        if check_attempt == max_checks - 1 {
                                            warn!("MCP server '{}' readiness timed out", server_name);
                                        }
                                    }
                                }
                            }

                            Ok((server_name.clone(), server_id.clone()))
                        }
                        Ok(Err(e)) => {
                            warn!("{}", format_error(templates::FAILED_TO_START, &format!("MCP server '{}'", server_name), &e));
                            Err((server_name.clone(), server_id.clone(), e))
                        }
                        Err(_) => {
                            warn!("MCP server '{}' startup timed out", server_name);
                            Err((server_name.clone(), server_id.clone(), "timeout".to_string()))
                        }
                    }
                })
            })
            .collect();

        let startup_start = std::time::Instant::now();
        let results = futures::future::join_all(startup_tasks).await;
        let total_startup_time = startup_start.elapsed();

        let mut successful_count = 0usize;
        let mut failed_server_ids: Vec<String> = Vec::new();
        let mut failed_names: Vec<String> = Vec::new();

        for result in results {
            match result {
                Ok(Ok(_)) => successful_count += 1,
                Ok(Err((name, id, _err))) => {
                    failed_names.push(name);
                    failed_server_ids.push(id);
                }
                Err(e) => {
                    warn!("MCP startup task join error: {}", e);
                }
            }
        }

        if successful_count > 0 || !failed_names.is_empty() {
            info!(
                "MCP init: {} ok, {} failed ({}ms)",
                successful_count,
                failed_names.len(),
                total_startup_time.as_millis()
            );
        }

        // Auto-remove servers that failed to start — purge from config entirely.
        // Hold the ToolConfigManager lock across both removal and save to prevent races.
        if !failed_server_ids.is_empty() {
            warn!("Removing failed MCP servers: {} (re-add in Settings if needed)", failed_names.join(", "));

            // Remove from MCP manager first (doesn't need ToolConfigManager lock)
            {
                let manager_guard = manager.lock().await;
                for failed_id in &failed_server_ids {
                    if let Err(e) = manager_guard.remove_server(failed_id).await {
                        debug!("Failed to remove server '{}' from MCP manager: {}", failed_id, e);
                    }
                }
            }

            let config_manager = self.get_tool_config_manager().await;
            let mut config_guard = config_manager.lock().await;
            for failed_id in &failed_server_ids {
                config_guard.remove_mcp_server(failed_id);
            }

            if let Some(handle) = app_handle {
                match crate::settings::manager::SettingsManager::new(handle.clone()) {
                    Ok(settings_manager) => {
                        if let Err(e) = config_guard.save_to_centralized_settings(&settings_manager).await {
                            warn!("Failed to persist MCP server removal: {}", e);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to create settings manager for MCP cleanup: {}", e);
                    }
                }
            }
            drop(config_guard);
        }

        // Sync tools once at the end (unchanged)
        if let Err(e) = self.sync_mcp_tools().await {
            warn!("Failed to sync MCP tools after initialization: {}", e);
        }

        Ok(())
    }

    /// Retry failed MCP servers with exponential backoff
    pub async fn retry_failed_mcp_servers(&self) -> Result<(), String> {
        let mcp_manager = self.get_mcp_manager().await;
        let manager_guard = mcp_manager.lock().await;

        let statuses = manager_guard.get_server_statuses().await;
        let failed_servers: Vec<String> = statuses
            .iter()
            .filter_map(|(id, status)| match status {
                MCPServerStatus::Error(_) | MCPServerStatus::Timeout => Some(id.clone()),
                _ => None,
            })
            .collect();

        if failed_servers.is_empty() {
            debug!("No failed MCP servers to retry");
            return Ok(());
        }

        info!(
            "Attempting to retry {} failed MCP servers",
            failed_servers.len()
        );

        let mut retry_count = 0;
        for server_id in failed_servers {
            // Each server manages its own backoff timing
            match manager_guard.start_server(&server_id).await {
                Ok(_) => {
                    info!("✅ Successfully retried MCP server: {}", server_id);
                    retry_count += 1;
                }
                Err(e) => {
                    debug!("⏭️ MCP server {} not ready for retry: {}", server_id, e);
                }
            }
        }

        if retry_count > 0 {
            info!("Retried {} MCP servers, syncing tools", retry_count);
            drop(manager_guard);
            self.sync_mcp_tools().await?;
        } else {
            drop(manager_guard);
        }

        Ok(())
    }

    /// Sync MCP tools with tool configuration
    pub async fn sync_mcp_tools(&self) -> Result<(), String> {
        let mcp_manager = self.get_mcp_manager().await;
        let manager_guard = mcp_manager.lock().await;
        let all_tools = manager_guard.get_all_tools().await;
        drop(manager_guard);

        let mut config_guard = self.tool_config_manager.lock().await;

        // Group tools by server
        let mut tools_by_server: HashMap<String, Vec<_>> = HashMap::new();
        for tool_info in all_tools {
            tools_by_server
                .entry(tool_info.server_id.clone())
                .or_insert_with(Vec::new)
                .push(tool_info);
        }

        // Add tools to configuration
        for (server_id, tools) in tools_by_server {
            config_guard.add_mcp_tools(&server_id, tools);
        }

        drop(config_guard);

        // Emit an event to trigger tool provider refresh in active agents
        self.notify_mcp_tools_updated().await;

        // Refresh all registered tool providers with the new MCP tools
        self.refresh_all_tool_providers().await?;

        Ok(())
    }

    /// Notify that MCP tools have been updated - this triggers a refresh in active agents
    /// NOTE: This is currently a stub awaiting implementation. It does not actually emit
    /// any event or notify agents. A proper implementation would need an AppHandle to
    /// emit events to the frontend and trigger tool provider refreshes.
    pub async fn notify_mcp_tools_updated(&self) {
        // Try to get an app handle and emit the event
        if let Ok(controller_guard) = self.browser_controller.try_lock() {
            if let Some(ref _controller) = *controller_guard {
                // Just emit without trying to get app handle from controller
                warn!("MCP tools updated notification is a stub - no event emitted to frontend");
            }
        }
    }

    /// Emit MCP state update to frontend
    pub async fn emit_mcp_state_update(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        let mcp_manager = self.get_mcp_manager().await;
        let manager_guard = mcp_manager.lock().await;

        let servers = manager_guard.get_server_configs().await;
        let statuses = manager_guard.get_server_statuses().await;
        let tools = manager_guard.get_all_tools().await;

        drop(manager_guard);

        let payload = serde_json::json!({
            "servers": servers,
            "statuses": statuses,
            "tools": tools
        });

        if let Err(e) = app_handle.emit(events::system::MCP_STATE_UPDATED, payload) {
            warn!("Failed to emit MCP state update: {}", e);
            return Err(format_error(templates::FAILED_TO_EMIT, "MCP state update", e));
        }

        debug!("Emitted MCP state update to frontend");
        Ok(())
    }

    /// Register a tool provider for MCP tool refresh notifications
    pub async fn register_tool_provider(
        &self,
        provider: Arc<TokioMutex<LocalToolProvider>>,
    ) -> Result<(), String> {
        let mut registry = self.tool_provider_registry.lock().await;
        registry.push(Arc::downgrade(&provider));
        debug!("Registered tool provider for MCP refresh notifications");
        Ok(())
    }

    /// Refresh all registered tool providers when MCP tools are updated
    pub async fn refresh_all_tool_providers(&self) -> Result<(), String> {
        let mut registry = self.tool_provider_registry.lock().await;

        // Filter out dead weak references and collect live ones
        let mut live_providers = Vec::new();
        registry.retain(|weak_provider| {
            if let Some(strong_provider) = weak_provider.upgrade() {
                live_providers.push(strong_provider);
                true // Keep this weak reference
            } else {
                debug!("Removing dead tool provider reference from registry");
                false // Remove this dead weak reference
            }
        });

        info!(
            "Refreshing {} live tool providers with updated MCP tools",
            live_providers.len()
        );

        // Release registry lock before async operations
        drop(registry);

        // Use proper async locks to ensure all providers are refreshed
        for provider_arc in live_providers {
            let mut provider = provider_arc.lock().await;
            if let Err(e) = provider.refresh_mcp_tools().await {
                warn!("Failed to refresh MCP tools for tool provider: {}", e);
            } else {
                debug!("Successfully refreshed MCP tools for tool provider");
            }
        }

        Ok(())
    }

    /// Cleanup all MCP servers and resources
    pub async fn cleanup_mcp_resources(&self) -> Result<(), String> {
        info!("🧹 Cleaning up MCP resources...");

        let mcp_manager = self.get_mcp_manager().await;
        let manager_guard = mcp_manager.lock().await;

        // Stop all servers
        let configs = manager_guard.get_server_configs().await;
        for config in configs {
            if let Err(e) = manager_guard.stop_server(&config.id).await {
                warn!("Failed to stop MCP server '{}': {}", config.name, e);
            }
        }

        drop(manager_guard);
        info!("✅ MCP resources cleaned up");
        Ok(())
    }

    /// Initialize MCP servers with atomic deduplication
    pub async fn initialize_mcp_servers_once(&self, app_handle: Option<&tauri::AppHandle>) -> Result<(), String> {
        use std::sync::atomic::{AtomicBool, Ordering};
        use tokio::sync::Mutex as AsyncMutex;

        static INIT_FLAG: AtomicBool = AtomicBool::new(false);
        static INIT_RESULT: std::sync::OnceLock<AsyncMutex<Option<Result<(), String>>>> =
            std::sync::OnceLock::new();

        // Fast path: if already initialized, return immediately
        if INIT_FLAG.load(Ordering::Acquire) {
            let result_mutex = INIT_RESULT.get_or_init(|| AsyncMutex::new(None));
            let guard = result_mutex.lock().await;
            return guard
                .as_ref()
                .ok_or_else(|| "MCP initialization result is None".to_string())?
                .clone();
        }

        // Try to claim initialization responsibility
        if INIT_FLAG
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            // We're responsible for initialization
            info!("Starting MCP servers initialization (first time)");
            let init_result = self.initialize_mcp_servers(app_handle).await;

            // Store the result
            let result_mutex = INIT_RESULT.get_or_init(|| AsyncMutex::new(None));
            let mut guard = result_mutex.lock().await;
            *guard = Some(init_result.clone());

            init_result
        } else {
            // Someone else is/was initializing, wait for their result
            debug!("MCP servers initialization already in progress, waiting for result");
            let result_mutex = INIT_RESULT.get_or_init(|| AsyncMutex::new(None));

            // Wait for initialization to complete
            loop {
                let guard = result_mutex.lock().await;
                if let Some(result) = guard.as_ref() {
                    return result.clone();
                }
                drop(guard);

                // Brief yield to avoid busy waiting
                tokio::task::yield_now().await;
            }
        }
    }

    // ── Multi-agent cursor tracking (Phase 4) ────────────────────────────────

    /// Update or insert the cursor state for a given agent.
    pub fn update_agent_cursor(&self, cursor: AgentCursorState) {
        match self.agent_cursors.lock() {
            Ok(mut map) => { map.insert(cursor.agent_id.clone(), cursor); }
            Err(e) => warn!("Failed to update agent cursor: {}", e),
        }
    }

    /// Remove an agent's cursor (call when the agent finishes or is cancelled).
    pub fn remove_agent_cursor(&self, agent_id: &str) {
        match self.agent_cursors.lock() {
            Ok(mut map) => { map.remove(agent_id); }
            Err(e) => warn!("Failed to remove agent cursor: {}", e),
        }
    }

    /// Snapshot of all active agent cursors (cloned for safe cross-thread use).
    pub fn get_agent_cursors(&self) -> Vec<AgentCursorState> {
        self.agent_cursors
            .lock()
            .map(|map| map.values().cloned().collect())
            .unwrap_or_default()
    }

    // TTS content is now handled via XML tags during streaming, no separate methods needed

    // Debug mode methods
    pub fn set_debug_mode(&self, enabled: bool) -> Result<(), String> {
        self.ui_settings
            .lock()
            .map(|mut settings| settings.debug_mode = enabled)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "debug mode", e))
    }

    pub fn is_debug_mode(&self) -> bool {
        self.ui_settings
            .lock()
            .map(|settings| settings.debug_mode)
            .unwrap_or_else(|e| {
                error!("Failed to check debug mode status: {}", e);
                false // Safe fallback
            })
    }

    // Methods for tool approval setting
    pub fn set_tool_approval_required(&self, required: bool) -> Result<(), String> {
        self.agent_execution
            .lock()
            .map(|mut execution| execution.tool_approval_required = required)
            .map_err(|e| format_error(templates::FAILED_TO_SET, "tool approval required", e))
    }

    pub fn is_tool_approval_required(&self) -> bool {
        self.agent_execution
            .lock()
            .map(|execution| execution.tool_approval_required)
            .unwrap_or_else(|e| {
                error!("Failed to check tool approval requirement: {}", e);
                false // Safe fallback
            })
    }

    // Methods for managing tool approval requests
    pub async fn add_pending_tool_approval(&self, request: ToolApprovalRequest) {
        let mut pending_guard = self.pending_tool_approvals.lock().await;
        pending_guard.insert(request.tool_id.clone(), request);
    }

    pub async fn approve_tool(&self, tool_id: &str) -> bool {
        let mut pending_guard = self.pending_tool_approvals.lock().await;
        if let Some(request) = pending_guard.get_mut(tool_id) {
            request.approved = Some(true);
            true
        } else {
            false
        }
    }

    pub async fn deny_tool(&self, tool_id: &str) -> bool {
        let mut pending_guard = self.pending_tool_approvals.lock().await;
        if let Some(request) = pending_guard.get_mut(tool_id) {
            request.approved = Some(false);
            true
        } else {
            false
        }
    }

    pub async fn get_tool_approval_status(&self, tool_id: &str) -> Option<bool> {
        let pending_guard = self.pending_tool_approvals.lock().await;
        pending_guard
            .get(tool_id)
            .and_then(|request| request.approved)
    }

    pub async fn remove_tool_approval(&self, tool_id: &str) -> Option<ToolApprovalRequest> {
        let mut pending_guard = self.pending_tool_approvals.lock().await;
        pending_guard.remove(tool_id)
    }

    pub async fn get_pending_tool_approvals(&self) -> Vec<ToolApprovalRequest> {
        let pending_guard = self.pending_tool_approvals.lock().await;
        pending_guard.values().cloned().collect()
    }

    pub async fn clear_pending_tool_approvals(&self) {
        let mut pending_guard = self.pending_tool_approvals.lock().await;
        pending_guard.clear();
    }
}

// Helper function to update undo state
#[allow(dead_code)] // Keep allowing dead code as it might be conditionally used
pub(crate) fn update_undo_state(
    state: &AppState,
    file_path: PathBuf,
    previous_content: Option<String>,
) -> Result<(), String> {
    // Safely handle potential lock poisoning with proper error handling
    let mut last_edited = state
        .last_edited_file
        .lock()
        .map_err(|e| format_error(templates::FAILED_TO_ACCESS, "last_edited_file lock", e))?;
    *last_edited = Some(file_path);
    drop(last_edited);

    let mut previous = state
        .previous_content
        .lock()
        .map_err(|e| format_error(templates::FAILED_TO_ACCESS, "previous_content lock", e))?;
    *previous = Some(previous_content);

    Ok(())
}

// DesktopWrapper implementation moved to desktop_wrapper.rs

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn test_keyboard_shortcuts_default() {
        let shortcuts = KeyboardShortcuts::default();

        #[cfg(target_os = "macos")]
        {
            assert_eq!(shortcuts.agent_mode, defaults::AGENT_MODE);
            assert_eq!(shortcuts.dictation_input, defaults::DICTATION_INPUT);
            assert_eq!(shortcuts.open_settings, defaults::OPEN_SETTINGS);
        }

        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(shortcuts.agent_mode, defaults::AGENT_MODE);
            assert_eq!(shortcuts.dictation_input, defaults::DICTATION_INPUT);
            assert_eq!(shortcuts.open_settings, defaults::OPEN_SETTINGS);
        }

        assert_eq!(shortcuts.stop_current_task, defaults::STOP_CURRENT_TASK);
    }

    #[test]
    fn test_keyboard_shortcuts_serialization() {
        let shortcuts = KeyboardShortcuts::default();
        let serialized = serde_json::to_string(&shortcuts).unwrap();
        let deserialized: KeyboardShortcuts = serde_json::from_str(&serialized).unwrap();

        assert_eq!(shortcuts.agent_mode, deserialized.agent_mode);
        assert_eq!(shortcuts.dictation_input, deserialized.dictation_input);
        assert_eq!(shortcuts.stop_current_task, deserialized.stop_current_task);
        assert_eq!(shortcuts.open_settings, deserialized.open_settings);
    }

    #[test]
    fn test_timestamp_tracker_new() {
        let tracker = TimestampTracker::new();
        assert!(tracker.last_timestamp_shown.is_none());
        assert_eq!(tracker.events_since_last_timestamp, 0);
    }

    #[test]
    fn test_timestamp_tracker_record_event() {
        let mut tracker = TimestampTracker::new();

        // Record event with timestamp shown
        tracker.record_event(1000, true);
        assert_eq!(tracker.last_timestamp_shown, Some(1000));
        assert_eq!(tracker.events_since_last_timestamp, 0);

        // Record event without timestamp shown
        tracker.record_event(2000, false);
        assert_eq!(tracker.last_timestamp_shown, Some(1000)); // Should remain unchanged
        assert_eq!(tracker.events_since_last_timestamp, 1);

        // Record another event without timestamp
        tracker.record_event(3000, false);
        assert_eq!(tracker.events_since_last_timestamp, 2);

        // Record event with timestamp shown again
        tracker.record_event(4000, true);
        assert_eq!(tracker.last_timestamp_shown, Some(4000));
        assert_eq!(tracker.events_since_last_timestamp, 0);
    }

    #[tokio::test]
    async fn test_app_state_creation() {
        let state = AppState::new(None);

        // Test initial state values
        assert!(!state.is_agent_executing());
        assert!(state.get_current_agent_execution_id().is_none());
        assert!(!state.are_permissions_checked());
        assert!(!state.is_cloud_enabled());

        // Test getter methods - using unwrap in tests is acceptable
        assert_eq!(state.get_tts_provider().unwrap(), "system");
        assert!(!state.get_dictation_active().unwrap());
        assert!(state.get_sound_enabled().unwrap());
        assert!(!state.get_always_listening_active().unwrap());
    }

    #[tokio::test]
    async fn test_agent_execution_tracking() {
        let state = AppState::new(None);
        let execution_id = "test-execution-123".to_string();

        // Initially not executing
        assert!(!state.is_agent_executing());
        assert!(state.get_current_agent_execution_id().is_none());

        // Mark as started
        state
            .mark_agent_execution_started(execution_id.clone())
            .unwrap();
        assert!(state.is_agent_executing());
        assert_eq!(state.get_current_agent_execution_id(), Some(execution_id));

        // Mark as finished
        state.mark_agent_execution_finished();
        assert!(!state.is_agent_executing());
        assert!(state.get_current_agent_execution_id().is_none());
    }

    #[tokio::test]
    async fn test_cancellation_signaling() {
        let state = AppState::new(None);

        // Initially not cancelled
        assert!(!*state.cancel_rx.borrow());

        // Signal cancellation
        state.signal_cancel();

        // Should be cancelled now
        assert!(*state.cancel_rx.borrow());

        // Reset cancellation
        state.reset_cancel();

        // Should not be cancelled anymore
        assert!(!*state.cancel_rx.borrow());
    }

    #[tokio::test]
    async fn test_cancel_receiver_watch() {
        let state = AppState::new(None);
        let mut cancel_rx = state.cancel_rx.clone();

        // Test that we can watch for changes
        let initial_value = *cancel_rx.borrow();
        assert!(!initial_value);

        // Signal cancellation in a separate task
        let state_clone = state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            state_clone.signal_cancel();
        });

        // Wait for the change
        let changed = timeout(Duration::from_millis(100), cancel_rx.changed()).await;
        assert!(changed.is_ok());

        // Value should now be true
        assert!(*cancel_rx.borrow());
    }

    #[tokio::test]
    async fn test_memory_manager_access() {
        let state = AppState::new(None);

        let memory_manager = state.get_memory_manager().await;
        assert!(memory_manager.try_lock().is_ok());
    }

    #[test]
    fn test_permissions_tracking() {
        let state = AppState::new(None);

        // Initially not checked
        assert!(!state.are_permissions_checked());

        // Mark as checked
        state.mark_permissions_checked().unwrap();
        assert!(state.are_permissions_checked());
    }

    #[tokio::test]
    async fn test_permissions_state_update() {
        let state = AppState::new(None);

        // Initially no permissions state
        assert!(state.get_permissions_state().await.is_none());

        // Create a mock permissions state with correct structure
        use crate::commands::permissions::{PermissionStatus, PermissionsState};
        let mock_permissions = vec![
            PermissionStatus {
                permission_type: crate::constants::permissions::types::ACCESSIBILITY.to_string(),
                granted: true,
                required: true,
                description: "Accessibility permission is granted".to_string(),
                instructions: "No action needed".to_string(),
            },
            PermissionStatus {
                permission_type: crate::constants::permissions::types::SCREEN_RECORDING.to_string(),
                granted: false,
                required: true,
                description: "Screen recording permission is denied".to_string(),
                instructions: "Grant in System Preferences".to_string(),
            },
            PermissionStatus {
                permission_type: crate::constants::permissions::types::MICROPHONE.to_string(),
                granted: true,
                required: false,
                description: "Microphone permission not determined".to_string(),
                instructions: "Will prompt when needed".to_string(),
            },
            PermissionStatus {
                permission_type: crate::constants::permissions::types::INPUT_MONITORING.to_string(),
                granted: true,
                required: true,
                description: "Input monitoring permission is granted".to_string(),
                instructions: "No action needed".to_string(),
            },
        ];

        let permissions_state = PermissionsState {
            accessibility: mock_permissions[0].clone(),
            screen_recording: mock_permissions[1].clone(),
            microphone: mock_permissions[2].clone(),
            input_monitoring: mock_permissions[3].clone(),
            all_granted: false,
            app_name: "TestApp".to_string(),
        };

        // Update permissions state
        state
            .update_permissions_state(permissions_state.clone())
            .await;

        // Should now have the permissions state
        let retrieved_state = state.get_permissions_state().await;
        assert!(retrieved_state.is_some());
        let retrieved = retrieved_state.unwrap();
        assert_eq!(retrieved.accessibility.granted, true);
        assert_eq!(retrieved.screen_recording.granted, false);
        assert_eq!(retrieved.all_granted, false);
        assert_eq!(retrieved.app_name, "TestApp");
    }

    #[test]
    fn test_desktop_availability() {
        let state = AppState::new(None);

        // With no desktop instance provided, should not be available
        assert!(!state.is_desktop_available());
        assert!(state.try_get_desktop().is_none());
        assert!(state.get_desktop().is_err());
    }

    #[test]
    fn test_state_components_storage() {
        let state = AppState::new(None);

        // Test inserting and retrieving a component
        #[derive(Clone, Debug, PartialEq)]
        struct TestComponent {
            value: String,
        }

        let test_component = TestComponent {
            value: "test_value".to_string(),
        };

        // Insert component
        state.insert(test_component.clone()).unwrap();

        // Retrieve component
        let retrieved = state.get::<TestComponent>();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, "test_value");

        // Try to retrieve non-existent component
        #[derive(Clone)]
        struct OtherComponent;

        let other = state.get::<OtherComponent>();
        assert!(other.is_none());
    }

    #[test]
    fn test_tts_provider_management() {
        let state = AppState::new(None);

        // Check initial value
        assert_eq!(state.get_tts_provider().unwrap(), "system");

        // Update TTS provider using new setter method
        state.set_tts_provider("openai".to_string()).unwrap();

        // Verify update persists
        assert_eq!(state.get_tts_provider().unwrap(), "openai");
    }

    #[test]
    fn test_dictation_state_management() {
        let state = AppState::new(None);

        // Check initial state
        assert!(!state.get_dictation_active().unwrap());
        assert!(state.get_dictation_clipboard_enabled().unwrap());

        // Update dictation state using new setter methods
        state.set_dictation_active(true).unwrap();
        state.set_dictation_clipboard_enabled(false).unwrap();

        // Verify updates persist
        assert!(state.get_dictation_active().unwrap());
        assert!(!state.get_dictation_clipboard_enabled().unwrap());
    }

    #[test]
    fn test_always_listening_configuration() {
        let state = AppState::new(None);

        // Check initial values
        assert!(!state.get_always_listening_active().unwrap());
        assert_eq!(state.get_always_listening_sensitivity().unwrap(), 0.5);

        let wake_words = state.get_always_listening_wake_words().unwrap();
        assert_eq!(wake_words.len(), 2);
        assert!(wake_words.contains(&audio::DEFAULT_WAKE_WORDS[0].to_string()));
        assert!(wake_words.contains(&audio::DEFAULT_WAKE_WORDS[1].to_string()));

        // Update configuration using new setter methods
        state.set_always_listening_active(true).unwrap();
        state.set_always_listening_sensitivity(0.8).unwrap();

        let mut new_wake_words = wake_words;
        new_wake_words.push("assistant".to_string());
        state
            .set_always_listening_wake_words(new_wake_words)
            .unwrap();

        // Verify updates persist
        assert!(state.get_always_listening_active().unwrap());
        assert_eq!(state.get_always_listening_sensitivity().unwrap(), 0.8);

        let updated_wake_words = state.get_always_listening_wake_words().unwrap();
        assert_eq!(updated_wake_words.len(), 3);
        assert!(updated_wake_words.contains(&"assistant".to_string()));
    }

    #[test]
    fn test_app_state_clone() {
        let state1 = AppState::new(None);
        let state2 = state1.clone();

        // Both should track the same agent execution state
        state1
            .mark_agent_execution_started("test-123".to_string())
            .unwrap();
        assert!(state2.is_agent_executing());
        assert_eq!(
            state2.get_current_agent_execution_id(),
            Some("test-123".to_string())
        );

        // Both should respond to cancellation signals
        state1.signal_cancel();
        assert!(*state2.cancel_rx.borrow());
    }

    #[tokio::test]
    async fn test_browser_controller_lazy_initialization() {
        let state = AppState::new(None);

        // Initially no browser controller
        {
            let controller_guard = state.browser_controller.lock().await;
            assert!(controller_guard.is_none());
        }

        // get_or_init_browser_controller should not panic; in CI it may succeed
        // depending on environment. Assert non-panic and allow either outcome.
        let _ = state.get_or_init_browser_controller().await;
    }

    #[tokio::test]
    async fn test_cloud_configuration() {
        let state = AppState::new(None);

        // Initially cloud should be disabled
        assert!(!state.is_cloud_enabled());

        // Get initial cloud config
        let _ = state.get_cloud_config().await;
    }
}
