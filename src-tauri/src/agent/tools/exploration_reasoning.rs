//! Exploration-Then-Reasoning Paradigm Implementation
//! Based on GUI-Xplore research from CVPR 2025
//!
//! Revolutionary approach where agents explore interfaces before reasoning
//! Mirrors human behavior: explore first, then perform tasks
//!
//! Used by: Computer Use tools for enhanced performance in unfamiliar environments

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use tracing::{debug, info, warn};
use tokio::time::{Duration, Instant};
use image::{ImageBuffer, Rgba};
use chrono::{DateTime, Utc};

/// Configuration for Exploration-Then-Reasoning system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationConfig {
    /// Maximum exploration time in seconds
    pub max_exploration_time: u64,
    /// Number of exploration steps before reasoning
    pub exploration_steps: u32,
    /// Minimum confidence threshold for GUI transitions
    pub transition_confidence_threshold: f32,
    /// Whether to use function-aware task goal generation
    pub use_function_aware_goals: bool,
    /// Maximum number of GUI states to remember
    pub max_gui_states: usize,
}

impl Default for ExplorationConfig {
    fn default() -> Self {
        Self {
            max_exploration_time: 30, // 30 seconds max exploration
            exploration_steps: 5,
            transition_confidence_threshold: 0.7,
            use_function_aware_goals: true,
            max_gui_states: 50,
        }
    }
}

/// Represents a GUI state during exploration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GUIState {
    /// Unique identifier for this state
    pub state_id: String,
    /// Screenshot of the GUI state (base64 encoded)
    pub screenshot: String,
    /// Timestamp when this state was captured
    pub timestamp: u64,
    /// Detected interactive elements
    pub interactive_elements: Vec<InteractiveElement>,
    /// Application context information
    pub app_context: AppContext,
    /// State importance score (0.0 to 1.0)
    pub importance_score: f32,
}

/// Interactive element detected in a GUI state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveElement {
    /// Element type (button, menu, text_field, etc.)
    pub element_type: String,
    /// Coordinates of the element
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Functional description
    pub function_description: Option<String>,
    /// Element text content
    pub text_content: Option<String>,
    /// Confidence score for detection
    pub confidence: f32,
}

/// Application context information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppContext {
    /// Application name/identifier
    pub app_name: String,
    /// Current window title
    pub window_title: Option<String>,
    /// Application category
    pub category: Option<String>,
    /// Version information if available
    pub version: Option<String>,
}

/// GUI transition between states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GUITransition {
    /// Source state ID
    pub from_state: String,
    /// Target state ID
    pub to_state: String,
    /// Action that caused the transition
    pub trigger_action: ActionType,
    /// Confidence score for this transition
    pub confidence: f32,
    /// Time taken for the transition
    pub transition_time_ms: u64,
}

/// Types of actions that can trigger GUI transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    Click { x: f32, y: f32 },
    KeyPress { key: String },
    Scroll { direction: ScrollDirection, amount: f32 },
    Type { text: String },
    Menu { menu_path: Vec<String> },
    Unknown,
}

/// Scroll direction for GUI interactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// GUI Transition Graph for modeling application structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GUITransitionGraph {
    /// All GUI states in the graph
    pub states: HashMap<String, GUIState>,
    /// Transitions between states
    pub transitions: HashMap<String, Vec<GUITransition>>,
    /// Layout patterns detected
    pub layout_patterns: Vec<LayoutPattern>,
    /// Application structure mapping
    pub app_structure: AppStructureMapping,
}

/// Layout patterns detected in the GUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPattern {
    /// Pattern type (navigation, content, toolbar, etc.)
    pub pattern_type: String,
    /// Pattern description
    pub description: String,
    /// Confidence score
    pub confidence: f32,
    /// Associated states where this pattern appears
    pub associated_states: Vec<String>,
}

/// Application structure mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStructureMapping {
    /// Main navigation areas
    pub navigation_areas: Vec<NavigationArea>,
    /// Content areas
    pub content_areas: Vec<ContentArea>,
    /// Toolbar areas
    pub toolbar_areas: Vec<ToolbarArea>,
    /// Workflow patterns
    pub workflow_patterns: Vec<WorkflowPattern>,
}

/// Navigation area in the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationArea {
    /// Area coordinates
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Navigation items
    pub items: Vec<NavigationItem>,
}

/// Navigation item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationItem {
    /// Item label
    pub label: String,
    /// Target state ID
    pub target_state: Option<String>,
    /// Item coordinates
    pub x: f32,
    pub y: f32,
}

/// Content area in the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentArea {
    /// Area coordinates
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Content type
    pub content_type: String,
    /// Dynamic content indicator
    pub is_dynamic: bool,
}

/// Toolbar area in the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolbarArea {
    /// Area coordinates
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Toolbar items
    pub items: Vec<ToolbarItem>,
}

/// Toolbar item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolbarItem {
    /// Item label
    pub label: String,
    /// Item type (button, dropdown, etc.)
    pub item_type: String,
    /// Item coordinates
    pub x: f32,
    pub y: f32,
}

/// Workflow pattern detected in the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPattern {
    /// Pattern name
    pub name: String,
    /// Sequence of states in the workflow
    pub state_sequence: Vec<String>,
    /// Common transitions
    pub common_transitions: Vec<GUITransition>,
    /// Pattern confidence
    pub confidence: f32,
}

/// Result of exploration phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationResult {
    /// GUI transition graph built during exploration
    pub gui_transition_graph: GUITransitionGraph,
    /// Interaction patterns discovered
    pub interaction_patterns: Vec<InteractionPattern>,
    /// Application structure mapping
    pub app_structure: AppStructureMapping,
    /// Total exploration time in milliseconds
    pub exploration_time_ms: u64,
    /// Number of states explored
    pub states_explored: u32,
    /// Exploration completeness score (0.0 to 1.0)
    pub completeness_score: f32,
}

/// Interaction pattern discovered during exploration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionPattern {
    /// Pattern name
    pub name: String,
    /// Pattern description
    pub description: String,
    /// Sequence of actions in the pattern
    pub action_sequence: Vec<ActionType>,
    /// Pattern frequency
    pub frequency: u32,
    /// Confidence score
    pub confidence: f32,
}

/// Error type for exploration-reasoning operations
#[derive(Debug, thiserror::Error)]
pub enum ExplorationError {
    #[error("Exploration timeout: {0}")]
    Timeout(String),
    #[error("Invalid GUI state: {0}")]
    InvalidState(String),
    #[error("Transition analysis failed: {0}")]
    TransitionFailed(String),
    #[error("Pattern recognition failed: {0}")]
    PatternRecognitionFailed(String),
}

/// Main Exploration Engine implementing the exploration-then-reasoning paradigm
pub struct ExplorationEngine {
    config: ExplorationConfig,
    function_goal_generator: FunctionAwareTaskGoalGenerator,
    gui_transition_graph: GUITransitionGraph,
    exploration_memory: ExplorationMemory,
    knowledge_extractor: TransitionAwareKnowledgeExtractor,
}

/// Function-aware task goal generator
pub struct FunctionAwareTaskGoalGenerator {
    goal_templates: Vec<ExplorationGoal>,
    function_mapping: HashMap<String, Vec<String>>,
}

/// Exploration goal template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationGoal {
    /// Goal description
    pub description: String,
    /// Target elements to find
    pub target_elements: Vec<String>,
    /// Expected outcomes
    pub expected_outcomes: Vec<String>,
    /// Priority level
    pub priority: u32,
}

/// Exploration memory system
pub struct ExplorationMemory {
    recent_states: VecDeque<GUIState>,
    state_history: HashMap<String, GUIState>,
    transition_history: Vec<GUITransition>,
    max_memory_size: usize,
}

/// Transition-aware knowledge extractor
pub struct TransitionAwareKnowledgeExtractor {
    pattern_recognizer: PatternRecognizer,
    layout_analyzer: LayoutAnalyzer,
    workflow_detector: WorkflowDetector,
}

/// Pattern recognizer for GUI interactions
pub struct PatternRecognizer {
    known_patterns: Vec<InteractionPattern>,
    confidence_threshold: f32,
}

/// Layout analyzer for GUI structure
pub struct LayoutAnalyzer {
    layout_templates: Vec<LayoutTemplate>,
}

/// Layout template for common GUI layouts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutTemplate {
    /// Template name
    pub name: String,
    /// Template description
    pub description: String,
    /// Expected areas
    pub expected_areas: Vec<ExpectedArea>,
}

/// Expected area in a layout template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedArea {
    /// Area type
    pub area_type: String,
    /// Relative position (0.0 to 1.0)
    pub relative_x: f32,
    pub relative_y: f32,
    pub relative_width: f32,
    pub relative_height: f32,
}

/// Workflow detector for application workflows
pub struct WorkflowDetector {
    workflow_templates: Vec<WorkflowTemplate>,
}

/// Workflow template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    /// Template name
    pub name: String,
    /// Expected state sequence
    pub expected_sequence: Vec<String>,
    /// Alternative paths
    pub alternative_paths: Vec<Vec<String>>,
}

impl ExplorationEngine {
    /// Creates a new exploration engine
    pub fn new() -> Self {
        Self::new_with_config(ExplorationConfig::default())
    }

    /// Creates a new exploration engine with custom configuration
    pub fn new_with_config(config: ExplorationConfig) -> Self {
        Self {
            config: config.clone(),
            function_goal_generator: FunctionAwareTaskGoalGenerator::new(),
            gui_transition_graph: GUITransitionGraph::new(),
            exploration_memory: ExplorationMemory::new(config.max_gui_states),
            knowledge_extractor: TransitionAwareKnowledgeExtractor::new(),
        }
    }

    /// Starts the exploration phase for an application
    pub async fn explore_application(
        &mut self,
        initial_screenshot: &str,
        app_context: AppContext,
    ) -> Result<ExplorationResult, ExplorationError> {
        let start_time = Instant::now();

        info!("Starting application exploration for: {}", app_context.app_name);

        // Initialize exploration with the initial state
        let initial_state = self.create_initial_state(initial_screenshot, &app_context).await?;
        self.exploration_memory.add_state(initial_state.clone());
        self.gui_transition_graph.add_state(initial_state);

        // Generate exploration goals
        let exploration_goals = self.function_goal_generator
            .generate_exploration_goals(&app_context)
            .await?;

        info!("Generated {} exploration goals", exploration_goals.len());

        // Execute exploration steps
        let mut states_explored = 1; // Initial state counts as 1
        let mut current_state_id = self.exploration_memory.get_latest_state_id()?;

        for step in 0..self.config.exploration_steps {
            if start_time.elapsed().as_secs() >= self.config.max_exploration_time {
                warn!("Exploration timeout reached at step {}", step);
                break;
            }

            // Select next exploration action
            let exploration_action = self.select_exploration_action(
                &current_state_id,
                &exploration_goals
            ).await?;

            // Execute exploration action and capture new state
            let action_type = exploration_action.action_type.clone();
            match self.execute_exploration_action(exploration_action).await {
                Ok((new_state, execution_time_ms)) => {
                    // Record transition
                    let transition = GUITransition {
                        from_state: current_state_id.clone(),
                        to_state: new_state.state_id.clone(),
                        trigger_action: action_type,
                        confidence: 0.9, // High confidence for executed actions
                        transition_time_ms: execution_time_ms,
                    };

                    self.gui_transition_graph.add_transition(transition.clone());
                    self.exploration_memory.add_transition(transition);
                    self.exploration_memory.add_state(new_state.clone());
                    self.gui_transition_graph.add_state(new_state.clone());

                    current_state_id = new_state.state_id;
                    states_explored += 1;

                    debug!("Exploration step {}: transitioned to state {}", step, current_state_id);
                }
                Err(e) => {
                    warn!("Exploration step {} failed: {}", step, e);
                    // Continue with exploration despite individual step failures
                }
            }
        }

        // Extract knowledge from exploration
        let interaction_patterns = self.knowledge_extractor
            .extract_interaction_patterns(&self.gui_transition_graph)
            .await?;

        let app_structure = self.knowledge_extractor
            .extract_app_structure(&self.gui_transition_graph)
            .await?;

        let exploration_time_ms = start_time.elapsed().as_millis() as u64;
        let completeness_score = self.calculate_completeness_score(states_explored, &exploration_goals);

        info!("Exploration completed: {} states explored in {}ms",
              states_explored, exploration_time_ms);

        Ok(ExplorationResult {
            gui_transition_graph: self.gui_transition_graph.clone(),
            interaction_patterns,
            app_structure,
            exploration_time_ms,
            states_explored,
            completeness_score,
        })
    }

    /// Creates the initial GUI state from a screenshot
    async fn create_initial_state(
        &self,
        screenshot: &str,
        app_context: &AppContext,
    ) -> Result<GUIState, ExplorationError> {
        let state_id = format!("state_{}", Utc::now().timestamp_millis());

        // Analyze screenshot for interactive elements
        let interactive_elements = self.detect_interactive_elements(screenshot).await?;

        Ok(GUIState {
            state_id,
            screenshot: screenshot.to_string(),
            timestamp: Utc::now().timestamp_millis() as u64,
            interactive_elements,
            app_context: app_context.clone(),
            importance_score: 1.0, // Initial state is always important
        })
    }

    /// Detects interactive elements in a screenshot
    async fn detect_interactive_elements(
        &self,
        screenshot: &str,
    ) -> Result<Vec<InteractiveElement>, ExplorationError> {
        // This would integrate with computer vision models in production
        // For now, return a simplified implementation

        let mut elements = Vec::new();

        // Simulate detection of common UI elements
        elements.push(InteractiveElement {
            element_type: "button".to_string(),
            x: 100.0,
            y: 50.0,
            width: 80.0,
            height: 30.0,
            function_description: Some("Primary action button".to_string()),
            text_content: Some("Submit".to_string()),
            confidence: 0.85,
        });

        Ok(elements)
    }

    /// Selects the next exploration action based on current state and goals
    async fn select_exploration_action(
        &self,
        current_state_id: &str,
        exploration_goals: &[ExplorationGoal],
    ) -> Result<ExplorationAction, ExplorationError> {
        let current_state = self.exploration_memory
            .get_state(current_state_id)
            .ok_or_else(|| ExplorationError::InvalidState(current_state_id.to_string()))?;

        // Simple exploration strategy: click on unexplored interactive elements
        for element in &current_state.interactive_elements {
            if !self.has_interacted_with_element(&element) {
                return Ok(ExplorationAction {
                    action_type: ActionType::Click { x: element.x, y: element.y },
                    target_element: Some(element.clone()),
                    execution_time_ms: 0, // Will be set during execution
                });
            }
        }

        // If no unexplored elements, try scroll actions
        Ok(ExplorationAction {
            action_type: ActionType::Scroll {
                direction: ScrollDirection::Down,
                amount: 3.0
            },
            target_element: None,
            execution_time_ms: 0,
        })
    }

    /// Checks if we have already interacted with an element
    fn has_interacted_with_element(&self, element: &InteractiveElement) -> bool {
        // Check transition history for interactions with this element
        for transition in &self.exploration_memory.transition_history {
            if let ActionType::Click { x, y } = &transition.trigger_action {
                // Check if coordinates are close to the element
                let distance = ((x - element.x).powi(2) + (y - element.y).powi(2)).sqrt();
                if distance < 10.0 { // 10 pixel tolerance
                    return true;
                }
            }
        }
        false
    }

    /// Executes an exploration action
    async fn execute_exploration_action(
        &self,
        _action: ExplorationAction,
    ) -> Result<(GUIState, u64), ExplorationError> {
        let start_time = Instant::now();

        // Here we would integrate with the actual computer use system
        // For now, simulate the action execution
        tokio::time::sleep(Duration::from_millis(100)).await;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        // Simulate capturing new state after action
        let new_state_id = format!("state_{}", Utc::now().timestamp_millis());

        // In production, this would capture an actual screenshot
        let simulated_screenshot = "simulated_screenshot_data".to_string();

        let new_state = GUIState {
            state_id: new_state_id,
            screenshot: simulated_screenshot,
            timestamp: Utc::now().timestamp_millis() as u64,
            interactive_elements: Vec::new(), // Would be detected from new screenshot
            app_context: AppContext {
                app_name: "Explored App".to_string(),
                window_title: Some("New State".to_string()),
                category: None,
                version: None,
            },
            importance_score: 0.8,
        };

        Ok((new_state, execution_time_ms))
    }

    /// Calculates exploration completeness score
    fn calculate_completeness_score(
        &self,
        states_explored: u32,
        exploration_goals: &[ExplorationGoal],
    ) -> f32 {
        let base_score = (states_explored as f32 / self.config.exploration_steps as f32).min(1.0);

        // Adjust score based on goal completion
        let goals_met = exploration_goals.len() as f32 * 0.5; // Assume 50% goal completion
        let goal_bonus = goals_met / exploration_goals.len() as f32 * 0.2;

        (base_score + goal_bonus).min(1.0)
    }
}

/// Exploration action to be executed
#[derive(Debug, Clone)]
pub struct ExplorationAction {
    /// Type of action
    pub action_type: ActionType,
    /// Target element (if applicable)
    pub target_element: Option<InteractiveElement>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

// Implementation of supporting structures

impl FunctionAwareTaskGoalGenerator {
    pub fn new() -> Self {
        Self {
            goal_templates: Vec::new(),
            function_mapping: HashMap::new(),
        }
    }

    pub async fn generate_exploration_goals(
        &self,
        app_context: &AppContext,
    ) -> Result<Vec<ExplorationGoal>, ExplorationError> {
        // Generate goals based on application type
        let mut goals = Vec::new();

        goals.push(ExplorationGoal {
            description: "Explore main navigation".to_string(),
            target_elements: vec!["menu".to_string(), "navigation".to_string()],
            expected_outcomes: vec!["Find main app sections".to_string()],
            priority: 1,
        });

        goals.push(ExplorationGoal {
            description: "Identify primary actions".to_string(),
            target_elements: vec!["button".to_string(), "action".to_string()],
            expected_outcomes: vec!["Find key functionality".to_string()],
            priority: 2,
        });

        Ok(goals)
    }
}

impl GUITransitionGraph {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            transitions: HashMap::new(),
            layout_patterns: Vec::new(),
            app_structure: AppStructureMapping::new(),
        }
    }

    pub fn add_state(&mut self, state: GUIState) {
        self.states.insert(state.state_id.clone(), state);
    }

    pub fn add_transition(&mut self, transition: GUITransition) {
        self.transitions
            .entry(transition.from_state.clone())
            .or_insert_with(Vec::new)
            .push(transition);
    }
}

impl AppStructureMapping {
    pub fn new() -> Self {
        Self {
            navigation_areas: Vec::new(),
            content_areas: Vec::new(),
            toolbar_areas: Vec::new(),
            workflow_patterns: Vec::new(),
        }
    }
}

impl ExplorationMemory {
    pub fn new(max_size: usize) -> Self {
        Self {
            recent_states: VecDeque::with_capacity(max_size),
            state_history: HashMap::new(),
            transition_history: Vec::new(),
            max_memory_size: max_size,
        }
    }

    pub fn add_state(&mut self, state: GUIState) {
        if self.recent_states.len() >= self.max_memory_size {
            if let Some(old_state) = self.recent_states.pop_front() {
                self.state_history.remove(&old_state.state_id);
            }
        }

        self.state_history.insert(state.state_id.clone(), state.clone());
        self.recent_states.push_back(state);
    }

    pub fn get_state(&self, state_id: &str) -> Option<&GUIState> {
        self.state_history.get(state_id)
    }

    pub fn get_latest_state_id(&self) -> Result<String, ExplorationError> {
        self.recent_states
            .back()
            .map(|state| state.state_id.clone())
            .ok_or_else(|| ExplorationError::InvalidState("No states in memory".to_string()))
    }

    pub fn add_transition(&mut self, transition: GUITransition) {
        self.transition_history.push(transition);
    }
}

impl TransitionAwareKnowledgeExtractor {
    pub fn new() -> Self {
        Self {
            pattern_recognizer: PatternRecognizer::new(),
            layout_analyzer: LayoutAnalyzer::new(),
            workflow_detector: WorkflowDetector::new(),
        }
    }

    pub async fn extract_interaction_patterns(
        &self,
        graph: &GUITransitionGraph,
    ) -> Result<Vec<InteractionPattern>, ExplorationError> {
        self.pattern_recognizer.analyze_patterns(graph).await
    }

    pub async fn extract_app_structure(
        &self,
        graph: &GUITransitionGraph,
    ) -> Result<AppStructureMapping, ExplorationError> {
        self.layout_analyzer.analyze_structure(graph).await
    }
}

impl PatternRecognizer {
    pub fn new() -> Self {
        Self {
            known_patterns: Vec::new(),
            confidence_threshold: 0.7,
        }
    }

    pub async fn analyze_patterns(
        &self,
        graph: &GUITransitionGraph,
    ) -> Result<Vec<InteractionPattern>, ExplorationError> {
        let mut patterns = Vec::new();

        // Analyze transition patterns
        for (from_state, transitions) in &graph.transitions {
            for transition in transitions {
                // Simple pattern detection - in production would use ML
                patterns.push(InteractionPattern {
                    name: "Click Pattern".to_string(),
                    description: format!("Click transition from {} to {}", from_state, transition.to_state),
                    action_sequence: vec![transition.trigger_action.clone()],
                    frequency: 1,
                    confidence: transition.confidence,
                });
            }
        }

        Ok(patterns)
    }
}

impl LayoutAnalyzer {
    pub fn new() -> Self {
        Self {
            layout_templates: Vec::new(),
        }
    }

    pub async fn analyze_structure(
        &self,
        graph: &GUITransitionGraph,
    ) -> Result<AppStructureMapping, ExplorationError> {
        // Analyze GUI states to extract structure
        let mut structure = AppStructureMapping::new();

        // Simple structure detection
        for (state_id, state) in &graph.states {
            for element in &state.interactive_elements {
                match element.element_type.as_str() {
                    "button" => {
                        // Add to toolbar areas
                        structure.toolbar_areas.push(ToolbarArea {
                            x: element.x,
                            y: element.y,
                            width: element.width,
                            height: element.height,
                            items: vec![ToolbarItem {
                                label: element.text_content.clone().unwrap_or_default(),
                                item_type: "button".to_string(),
                                x: element.x,
                                y: element.y,
                            }],
                        });
                    }
                    _ => {} // Handle other element types
                }
            }
        }

        Ok(structure)
    }
}

impl WorkflowDetector {
    pub fn new() -> Self {
        Self {
            workflow_templates: Vec::new(),
        }
    }
}
