use regex::Regex;
use std::collections::HashMap;
use tracing::{debug, info};

use crate::agent::providers::anthropic::ToolChoice;

/// Intelligence system for determining when and how to force tool usage
/// based on user input patterns, context, and operational modes
pub struct ToolChoiceIntelligence {
    /// Pattern matchers for different tool categories
    pattern_matchers: HashMap<String, Vec<PatternMatcher>>,
    /// Context-aware decision rules
    context_rules: Vec<ContextRule>,
    /// Current operational mode
    mode: OperationalMode,
    /// Configuration settings
    config: IntelligenceConfig,
}

/// Different operational modes that affect tool choice behavior
#[derive(Debug, Clone, PartialEq)]
pub enum OperationalMode {
    /// General agent mode - balanced tool usage
    Agent,
    /// Voice command mode - aggressive tool forcing for clear commands
    Voice,
    /// Dictation mode - minimal tool forcing, focus on text input
    Dictation,
    /// Always listening mode - context-sensitive tool forcing
    AlwaysListening,
    /// Debug mode - enhanced logging and permissive tool usage
    Debug,
}

/// Configuration for tool choice intelligence behavior
#[derive(Debug, Clone)]
pub struct IntelligenceConfig {
    /// Enable aggressive tool forcing for clear action commands
    pub aggressive_action_forcing: bool,
    /// Enable voice command optimization
    pub voice_command_optimization: bool,
    /// Enable context-aware tool selection
    pub context_awareness: bool,
    /// Minimum confidence threshold for tool forcing (0.0 - 1.0)
    pub confidence_threshold: f32,
    /// Enable learning from user corrections
    pub adaptive_learning: bool,
}

impl Default for IntelligenceConfig {
    fn default() -> Self {
        Self {
            aggressive_action_forcing: true,
            voice_command_optimization: true,
            context_awareness: true,
            confidence_threshold: 0.7,
            adaptive_learning: false, // Disabled by default for privacy
        }
    }
}

/// Pattern matcher for specific tool forcing scenarios
#[derive(Debug, Clone)]
struct PatternMatcher {
    /// Regex pattern to match user input
    pattern: Regex,
    /// Tool name to force when pattern matches
    tool_name: String,
    /// Confidence level of this pattern (0.0 - 1.0)
    confidence: f32,
    /// Description of what this pattern matches
    description: String,
    /// Whether this pattern requires exact tool forcing or just suggests it
    force_mode: ForceMode,
}

/// How aggressively to force tool usage
#[derive(Debug, Clone, PartialEq)]
enum ForceMode {
    /// Force the specific tool - use tool_choice: {"type": "tool", "name": "..."}
    ForceSpecific,
    /// Force any tool - use tool_choice: {"type": "any"}
    ForceAny,
    /// Suggest tool usage but let Claude decide - use tool_choice: {"type": "auto"}
    Suggest,
}

/// Context-based decision rule
#[derive(Debug, Clone)]
struct ContextRule {
    /// Rule name for debugging
    name: String,
    /// Function to evaluate context and return tool choice
    evaluator: ContextEvaluator,
}

/// Context evaluation result
#[derive(Debug, Clone)]
enum ContextEvaluator {
    /// Check if previous message was a tool call
    PreviousToolCall,
    /// Check if user is asking for help with specific tool
    ToolHelp,
    /// Check if user is correcting a tool usage
    ToolCorrection,
}

impl ToolChoiceIntelligence {
    /// Create a new tool choice intelligence system
    pub fn new(mode: OperationalMode) -> Self {
        let mut intelligence = Self {
            pattern_matchers: HashMap::new(),
            context_rules: Vec::new(),
            mode,
            config: IntelligenceConfig::default(),
        };

        intelligence.initialize_patterns();
        intelligence.initialize_context_rules();
        intelligence
    }

    /// Create with custom configuration
    pub fn with_config(mode: OperationalMode, config: IntelligenceConfig) -> Self {
        let mut intelligence = Self {
            pattern_matchers: HashMap::new(),
            context_rules: Vec::new(),
            mode,
            config,
        };

        intelligence.initialize_patterns();
        intelligence.initialize_context_rules();
        intelligence
    }

    /// Analyze user input and determine appropriate tool choice
    pub fn analyze_input(&self, user_input: &str, context: &AnalysisContext) -> ToolChoiceDecision {
        debug!("Analyzing input for tool choice: '{}'", user_input);

        let mut decisions = Vec::new();

        // 1. Pattern-based analysis
        for (category, matchers) in &self.pattern_matchers {
            for matcher in matchers {
                if matcher.pattern.is_match(user_input) {
                    if matcher.confidence >= self.config.confidence_threshold {
                        decisions.push(ToolChoiceDecision {
                            tool_choice: match matcher.force_mode {
                                ForceMode::ForceSpecific => Some(ToolChoice::Tool {
                                    choice_type: "tool".to_string(),
                                    name: matcher.tool_name.clone(),
                                }),
                                ForceMode::ForceAny => Some(ToolChoice::Any),
                                ForceMode::Suggest => Some(ToolChoice::Auto),
                            },
                            confidence: matcher.confidence,
                            reasoning: format!(
                                "Pattern match in {}: {} ({})",
                                category, matcher.description, matcher.tool_name
                            ),
                            source: DecisionSource::Pattern,
                        });
                    }
                }
            }
        }

        // 2. Context-based analysis
        if self.config.context_awareness {
            for rule in &self.context_rules {
                if let Some(decision) = self.evaluate_context_rule(rule, context) {
                    decisions.push(decision);
                }
            }
        }

        // 3. Mode-specific adjustments
        let final_decision = self.apply_mode_adjustments(decisions, user_input, context);

        info!(
            "Tool choice decision: {:?} (confidence: {:.2})",
            final_decision.tool_choice, final_decision.confidence
        );

        final_decision
    }

    /// Initialize pattern matchers for different tool categories
    fn initialize_patterns(&mut self) {
        self.initialize_screenshot_patterns();
        self.initialize_click_patterns();
        self.initialize_keyboard_patterns();
        self.initialize_browser_patterns();
        self.initialize_file_patterns();
        self.initialize_desktop_patterns();
    }

    /// Helper method to create patterns from configurations with safe regex compilation
    fn create_patterns_from_configs(
        configs: Vec<(&str, &str, f32, &str, ForceMode)>
    ) -> Vec<PatternMatcher> {
        let mut patterns = Vec::new();
        
        for (pattern_str, tool_name, confidence, description, force_mode) in configs {
            match Regex::new(pattern_str) {
                Ok(pattern) => {
                    patterns.push(PatternMatcher {
                        pattern,
                        tool_name: tool_name.to_string(),
                        confidence,
                        description: description.to_string(),
                        force_mode,
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to compile pattern regex '{}': {}",
                        pattern_str, e
                    );
                }
            }
        }
        
        patterns
    }

    /// Initialize screenshot tool patterns
    fn initialize_screenshot_patterns(&mut self) {
        let pattern_configs = vec![
            (r"(?i)\b(take|capture|get|grab)\s+(a\s+)?(screenshot|screen\s*shot|screen\s*cap)\b", "screenshot", 0.9, "Direct screenshot command", ForceMode::ForceSpecific),
            (r"(?i)\b(show\s+me\s+the\s+screen|what('s|\s+is)\s+on\s+screen)\b", "screenshot", 0.8, "Implicit screenshot request", ForceMode::ForceSpecific),
            (r"(?i)\b(screen\s*shot|screenshot)\b", "screenshot", 0.7, "Screenshot keyword", ForceMode::Suggest),
        ];

        let patterns = Self::create_patterns_from_configs(pattern_configs);
        self.pattern_matchers.insert("screenshot".to_string(), patterns);
    }

    /// Initialize click tool patterns
    fn initialize_click_patterns(&mut self) {
        let pattern_configs = vec![
            (r"(?i)\b(click|press|tap)\s+(on\s+)?(the\s+)?(\w+\s+)?(button|link|icon|element)\b", "click", 0.9, "Direct click command", ForceMode::ForceSpecific),
            (r"(?i)\b(left\s+click|right\s+click|double\s+click)\b", "click", 0.9, "Specific click type", ForceMode::ForceSpecific),
            (r"(?i)\bclick\s+(here|there|this|that)\b", "click", 0.8, "Contextual click command", ForceMode::ForceSpecific),
        ];

        let patterns = Self::create_patterns_from_configs(pattern_configs);
        self.pattern_matchers.insert("click".to_string(), patterns);
    }

    /// Initialize keyboard input patterns
    fn initialize_keyboard_patterns(&mut self) {
        let force_mode_for_text = if self.mode == OperationalMode::Dictation {
            ForceMode::ForceSpecific
        } else {
            ForceMode::Suggest
        };
        
        let pattern_configs = vec![
            (r"(?i)\b(type|enter|input|write)\s+", "key", 0.8, "Text input command", force_mode_for_text),
            (r"(?i)\b(press|hit)\s+(the\s+)?(enter|return|space|tab|escape|esc)\s+(key|button)?\b", "key", 0.9, "Specific key press", ForceMode::ForceSpecific),
            (r"(?i)\b(keyboard\s+shortcut|hotkey|key\s+combo)\b", "key", 0.8, "Keyboard shortcut command", ForceMode::ForceSpecific),
        ];

        let patterns = Self::create_patterns_from_configs(pattern_configs);
        self.pattern_matchers.insert("keyboard".to_string(), patterns);
    }

    /// Initialize browser-specific patterns
    fn initialize_browser_patterns(&mut self) {
        let pattern_configs = vec![
            (r"(?i)\b(open|navigate|go\s+to)\s+(a\s+)?(browser|web\s*page|url|website)\b", "browser_navigate", 0.8, "Browser navigation command", ForceMode::ForceAny),
            (r"(?i)\b(refresh|reload)\s+(the\s+)?(page|browser)\b", "browser_refresh", 0.9, "Page refresh command", ForceMode::ForceSpecific),
        ];

        let patterns = Self::create_patterns_from_configs(pattern_configs);
        self.pattern_matchers.insert("browser".to_string(), patterns);
    }

    /// Initialize file operation patterns
    fn initialize_file_patterns(&mut self) {
        let pattern_configs = vec![
            (r"(?i)\b(open|read|view)\s+(a\s+|the\s+)?file\b", "str_replace_editor", 0.8, "File read command", ForceMode::ForceAny),
            (r"(?i)\b(save|write|create)\s+(a\s+|the\s+)?file\b", "str_replace_editor", 0.8, "File write command", ForceMode::ForceAny),
        ];

        let patterns = Self::create_patterns_from_configs(pattern_configs);
        self.pattern_matchers.insert("file".to_string(), patterns);
    }

    /// Initialize desktop interaction patterns
    fn initialize_desktop_patterns(&mut self) {
        let pattern_configs = vec![
            (r"(?i)\b(minimize|maximize|close)\s+(the\s+)?(window|app|application)\b", "window_control", 0.9, "Window control command", ForceMode::ForceAny),
            (r"(?i)\b(switch\s+to|focus\s+on)\s+(the\s+)?(\w+\s+)?(window|app|application)\b", "window_focus", 0.8, "Window focus command", ForceMode::ForceAny),
        ];

        let patterns = Self::create_patterns_from_configs(pattern_configs);
        self.pattern_matchers.insert("desktop".to_string(), patterns);
    }

    /// Initialize context-based decision rules
    fn initialize_context_rules(&mut self) {
        self.context_rules.push(ContextRule {
            name: "Previous tool call".to_string(),
            evaluator: ContextEvaluator::PreviousToolCall,
        });

        self.context_rules.push(ContextRule {
            name: "Tool help request".to_string(),
            evaluator: ContextEvaluator::ToolHelp,
        });

        self.context_rules.push(ContextRule {
            name: "Tool correction".to_string(),
            evaluator: ContextEvaluator::ToolCorrection,
        });
    }

    /// Evaluate a context rule
    fn evaluate_context_rule(
        &self,
        rule: &ContextRule,
        context: &AnalysisContext,
    ) -> Option<ToolChoiceDecision> {
        match &rule.evaluator {
            ContextEvaluator::PreviousToolCall => {
                if context.previous_was_tool_call {
                    Some(ToolChoiceDecision {
                        tool_choice: Some(ToolChoice::Auto),
                        confidence: 0.6,
                        reasoning: "Previous message was a tool call, allowing natural flow"
                            .to_string(),
                        source: DecisionSource::Context,
                    })
                } else {
                    None
                }
            }
            ContextEvaluator::ToolHelp => {
                // Look for help requests about tools
                None // Simplified for now
            }
            ContextEvaluator::ToolCorrection => {
                // Look for user corrections of tool usage
                None // Simplified for now
            }
        }
    }

    /// Apply mode-specific adjustments to tool choice decisions
    fn apply_mode_adjustments(
        &self,
        mut decisions: Vec<ToolChoiceDecision>,
        user_input: &str,
        _context: &AnalysisContext,
    ) -> ToolChoiceDecision {
        // Sort by confidence, highest first
        decisions.sort_by(|a, b| {
            b.confidence.partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Get the best decision or create default
        let mut best_decision = decisions
            .first()
            .cloned()
            .unwrap_or_else(|| ToolChoiceDecision {
                tool_choice: None,
                confidence: 0.0,
                reasoning: "No specific patterns matched".to_string(),
                source: DecisionSource::Default,
            });

        // Apply mode-specific adjustments
        match self.mode {
            OperationalMode::Voice => {
                // Voice commands should be more aggressive about tool forcing
                if best_decision.confidence > 0.5 {
                    best_decision.confidence = (best_decision.confidence * 1.2).min(1.0);
                    best_decision
                        .reasoning
                        .push_str(" (boosted for voice mode)");
                }
            }
            OperationalMode::Dictation => {
                // Dictation mode should be less aggressive except for clear text input
                if !user_input.to_lowercase().contains("type")
                    && !user_input.to_lowercase().contains("enter")
                {
                    best_decision.confidence *= 0.7;
                    best_decision
                        .reasoning
                        .push_str(" (reduced for dictation mode)");
                }
            }
            OperationalMode::AlwaysListening => {
                // Always listening should require higher confidence
                if best_decision.confidence < 0.8 {
                    best_decision.tool_choice = Some(ToolChoice::Auto);
                    best_decision
                        .reasoning
                        .push_str(" (conservative for always listening)");
                }
            }
            OperationalMode::Debug => {
                // Debug mode provides more information
                best_decision.reasoning.push_str(&format!(
                    " [DEBUG: {} candidates evaluated]",
                    decisions.len()
                ));
            }
            OperationalMode::Agent => {
                // Default agent behavior - no adjustments needed
            }
        }

        best_decision
    }

    /// Update operational mode
    pub fn set_mode(&mut self, mode: OperationalMode) {
        if self.mode != mode {
            info!(
                "Changing tool choice intelligence mode: {:?} -> {:?}",
                self.mode, mode
            );
            self.mode = mode;
            // Reinitialize patterns with new mode-specific settings
            self.pattern_matchers.clear();
            self.initialize_patterns();
        }
    }

    /// Update configuration
    pub fn update_config(&mut self, config: IntelligenceConfig) {
        self.config = config;
        // Reinitialize patterns with new configuration
        self.pattern_matchers.clear();
        self.initialize_patterns();
    }
}

/// Context information for tool choice analysis
#[derive(Debug, Clone)]
pub struct AnalysisContext {
    /// Whether the previous message was a tool call
    pub previous_was_tool_call: bool,
    /// The name of the last tool that was called
    pub last_tool_name: Option<String>,
    /// Whether there was an error in the last tool call
    pub last_tool_error: bool,
    /// Current conversation length
    pub conversation_length: usize,
    /// Available tools in current context
    pub available_tools: Vec<String>,
}

impl Default for AnalysisContext {
    fn default() -> Self {
        Self {
            previous_was_tool_call: false,
            last_tool_name: None,
            last_tool_error: false,
            conversation_length: 0,
            available_tools: Vec::new(),
        }
    }
}

/// Decision about tool choice made by the intelligence system
#[derive(Debug, Clone)]
pub struct ToolChoiceDecision {
    /// The recommended tool choice (None means use default behavior)
    pub tool_choice: Option<ToolChoice>,
    /// Confidence level in this decision (0.0 - 1.0)
    pub confidence: f32,
    /// Human-readable reasoning for this decision
    pub reasoning: String,
    /// Source of this decision
    pub source: DecisionSource,
}

/// Source of a tool choice decision
#[derive(Debug, Clone, PartialEq)]
pub enum DecisionSource {
    /// Pattern-based matching
    Pattern,
    /// Context-based analysis
    Context,
    /// Mode-specific adjustment
    Mode,
    /// Default/fallback decision
    Default,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screenshot_pattern_matching() {
        let intelligence = ToolChoiceIntelligence::new(OperationalMode::Agent);
        let context = AnalysisContext::default();

        let decision = intelligence.analyze_input("take a screenshot", &context);
        assert!(matches!(
            decision.tool_choice,
            Some(ToolChoice::Tool { .. })
        ));
        assert!(decision.confidence > 0.8);
    }

    #[test]
    fn test_click_pattern_matching() {
        let intelligence = ToolChoiceIntelligence::new(OperationalMode::Agent);
        let context = AnalysisContext::default();

        let decision = intelligence.analyze_input("click the button", &context);
        assert!(matches!(
            decision.tool_choice,
            Some(ToolChoice::Tool { .. })
        ));
        assert!(decision.confidence > 0.8);
    }

    #[test]
    fn test_voice_mode_confidence_boost() {
        let intelligence = ToolChoiceIntelligence::new(OperationalMode::Voice);
        let context = AnalysisContext::default();

        let decision = intelligence.analyze_input("screenshot", &context);
        assert!(decision.reasoning.contains("boosted for voice mode"));
    }

    #[test]
    fn test_dictation_mode_reduction() {
        let intelligence = ToolChoiceIntelligence::new(OperationalMode::Dictation);
        let context = AnalysisContext::default();

        let decision = intelligence.analyze_input("click something", &context);
        assert!(decision.reasoning.contains("reduced for dictation mode"));
    }
}
