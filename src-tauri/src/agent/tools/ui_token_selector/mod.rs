//! UI-Guided Visual Token Selection Module
//!
//! Implements the ShowUI paper's RGB connected graph approach for reducing computational costs
//! in GUI screenshot processing by 33% while maintaining 1.4x faster performance.
//!
//! Research Foundation: ShowUI Paper (arXiv:2411.17465)
//! - Treats screenshots as connected graphs using RGB color space
//! - Adaptively identifies redundant visual relationships
//! - Reduces token sequences from 1296 to 291 in sparse areas
//! - Enhanced for multi-monitor support with Juno's existing infrastructure
//!
//! Used by: Anthropic Computer Use tools for optimized screenshot processing

use std::sync::Arc;
use std::time::Instant;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

pub mod config;
pub mod rgb_analyzer;
pub mod token_reducer;
pub mod display_optimizer;
pub mod performance;

pub use config::TokenSelectionConfig;
pub use rgb_analyzer::RGBConnectedGraphAnalyzer;
pub use token_reducer::TokenReducer;
pub use display_optimizer::DisplayOptimizer;
pub use performance::{PerformanceTracker, PerformanceMetrics};

/// Errors that can occur during UI token selection processing
#[derive(Debug, thiserror::Error)]
pub enum TokenSelectionError {
    #[error("Image processing error: {0}")]
    ImageProcessing(String),

    #[error("RGB analysis error: {0}")]
    RGBAnalysis(String),

    #[error("Token reduction error: {0}")]
    TokenReduction(String),

    #[error("Display optimization error: {0}")]
    DisplayOptimization(String),

    #[error("Performance tracking error: {0}")]
    PerformanceTracking(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Multi-monitor error: {0}")]
    MultiMonitor(String),

    #[error("Processing error: {0}")]
    ProcessingError(String),
}

/// Represents a screenshot that has been processed with UI-Guided Visual Token Selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenizedScreenshot {
    /// Original image dimensions
    pub original_width: u32,
    pub original_height: u32,

    /// Optimized token representation
    pub tokens: Vec<VisualToken>,

    /// Token reduction statistics
    pub original_token_count: u32,
    pub reduced_token_count: u32,
    pub reduction_percentage: f32,

    /// Display information for multi-monitor support
    pub display_id: Option<u32>,
    pub display_bounds: Option<DisplayBounds>,

    /// Processing metadata
    pub processing_time_ms: u64,
    pub rgb_analysis_time_ms: u64,
    pub token_reduction_time_ms: u64,
}

/// Represents a visual token with spatial and color information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualToken {
    /// Token position in the image
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,

    /// RGB color information
    pub dominant_color: RGBColor,
    pub color_variance: f32,

    /// Token importance score (0.0-1.0)
    pub importance_score: f32,

    /// Token type classification
    pub token_type: TokenType,

    /// Connected graph information
    pub connected_tokens: Vec<u32>, // Indices of connected tokens
    pub redundancy_group: Option<u32>, // Group ID for redundant tokens
}

/// RGB color representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RGBColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Types of visual tokens based on UI analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenType {
    /// Interactive UI elements (buttons, links, etc.)
    Interactive,
    /// Text content
    Text,
    /// Background/decorative elements
    Background,
    /// UI chrome (borders, dividers, etc.)
    Chrome,
    /// Unknown/unclassified
    Unknown,
}

/// Display bounds information for multi-monitor support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Main UI Token Selector that orchestrates the token selection process
pub struct UITokenSelector {
    config: TokenSelectionConfig,
    rgb_analyzer: Arc<RGBConnectedGraphAnalyzer>,
    token_reducer: Arc<TokenReducer>,
    display_optimizer: Arc<DisplayOptimizer>,
    performance_tracker: Arc<PerformanceTracker>,
}

impl UITokenSelector {
    /// Creates a new UITokenSelector with the specified configuration
    pub fn new(config: TokenSelectionConfig) -> Result<Self, TokenSelectionError> {
        debug!("Initializing UITokenSelector with config: {:?}", config);

        let rgb_analyzer = Arc::new(RGBConnectedGraphAnalyzer::new(&config)?);
        let token_reducer = Arc::new(TokenReducer::new(&config)?);
        let display_optimizer = Arc::new(DisplayOptimizer::new(&config)?);
        let performance_tracker = Arc::new(PerformanceTracker::new());

        Ok(Self {
            config,
            rgb_analyzer,
            token_reducer,
            display_optimizer,
            performance_tracker,
        })
    }

    /// Creates a UITokenSelector with default configuration optimized for multi-monitor setups
    pub fn new_with_defaults() -> Result<Self, TokenSelectionError> {
        let config = TokenSelectionConfig::default_multi_monitor();
        Self::new(config)
    }

    /// Processes a screenshot and returns an optimized tokenized representation
    ///
    /// # Arguments
    /// * `image_data` - Raw image data (PNG/JPEG bytes)
    /// * `display_info` - Optional display information for multi-monitor optimization
    ///
    /// # Returns
    /// * `TokenizedScreenshot` with optimized token representation
    pub async fn process_screenshot(
        &self,
        image_data: &[u8],
        display_info: Option<DisplayInfo>,
    ) -> Result<TokenizedScreenshot, TokenSelectionError> {
        let start_time = Instant::now();

        info!("Processing screenshot with UI-Guided Visual Token Selection");
        debug!("Image data size: {} bytes, Display info: {:?}", image_data.len(), display_info);

        // Step 1: Parse and validate image
        let image = self.parse_image(image_data)?;
        let (width, height) = (image.width(), image.height());

        // Step 2: RGB Connected Graph Analysis
        let rgb_start = Instant::now();
        let rgb_graph = self.rgb_analyzer.analyze_image(&image).await
            .map_err(|e| TokenSelectionError::RGBAnalysis(e.to_string()))?;
        let rgb_analysis_time = rgb_start.elapsed();

        // Step 3: Token Reduction based on connected graph
        let token_start = Instant::now();
        let (tokens, reduction_stats) = self.token_reducer.reduce_tokens(&rgb_graph).await
            .map_err(|e| TokenSelectionError::TokenReduction(e.to_string()))?;
        let token_reduction_time = token_start.elapsed();

        // Step 4: Display-specific optimization (if multi-monitor info available)
        let optimized_tokens = if let Some(ref display) = display_info {
            self.display_optimizer.optimize_for_display(tokens, &display).await
                .map_err(|e| TokenSelectionError::DisplayOptimization(e.to_string()))?
        } else {
            tokens
        };

        let total_processing_time = start_time.elapsed();

        // Step 5: Create tokenized screenshot result
        let tokenized_screenshot = TokenizedScreenshot {
            original_width: width,
            original_height: height,
            tokens: optimized_tokens.clone(),
            original_token_count: reduction_stats.original_count,
            reduced_token_count: optimized_tokens.len() as u32,
            reduction_percentage: reduction_stats.reduction_percentage,
            display_id: display_info.as_ref().map(|d| d.id),
            display_bounds: display_info.as_ref().map(|d| DisplayBounds {
                x: d.bounds.x,
                y: d.bounds.y,
                width: d.bounds.width,
                height: d.bounds.height,
            }),
            processing_time_ms: total_processing_time.as_millis() as u64,
            rgb_analysis_time_ms: rgb_analysis_time.as_millis() as u64,
            token_reduction_time_ms: token_reduction_time.as_millis() as u64,
        };

        // Step 6: Update performance metrics
        self.performance_tracker.record_operation(
            reduction_stats.original_count,
            optimized_tokens.len() as u32,
            total_processing_time,
            display_info.as_ref().map(|d| d.id).unwrap_or(0),
            display_info.as_ref().map(|d| (d.bounds.width as u32, d.bounds.height as u32)).unwrap_or((width, height)),
            0.0, // TODO: Implement actual memory tracking
        ).map_err(|e| TokenSelectionError::PerformanceTracking(e.to_string()))?;

        info!(
            "Screenshot processing complete: {}x{} -> {} tokens ({:.1}% reduction) in {}ms",
            width, height,
            optimized_tokens.len(),
            reduction_stats.reduction_percentage,
            total_processing_time.as_millis()
        );

        Ok(tokenized_screenshot)
    }

    /// Gets current performance metrics
    pub fn get_performance_metrics(&self) -> Result<PerformanceMetrics, String> {
        self.performance_tracker.get_metrics()
    }

    /// Updates configuration (useful for runtime optimization)
    pub fn update_config(&mut self, new_config: TokenSelectionConfig) -> Result<(), TokenSelectionError> {
        debug!("Updating UITokenSelector configuration");

        // Validate new configuration
        new_config.validate()
            .map_err(|e| TokenSelectionError::Configuration(e))?;

        self.config = new_config;

        // Note: In a production system, we might want to recreate analyzers with new config
        // For now, we'll keep the existing ones to avoid disruption
        warn!("Configuration updated, but existing analyzers retain old config until restart");

        Ok(())
    }

    /// Gets current configuration
    pub fn get_config(&self) -> &TokenSelectionConfig {
        &self.config
    }

    /// Parses image data into a usable image format
    fn parse_image(&self, image_data: &[u8]) -> Result<image::DynamicImage, TokenSelectionError> {
        image::load_from_memory(image_data)
            .map_err(|e| TokenSelectionError::ImageProcessing(format!("Failed to parse image: {}", e)))
    }
}

/// Display information structure for multi-monitor support
/// Re-exported from display module for convenience
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub id: u32,
    pub bounds: DisplayBounds,
    pub is_main: bool,
}

/// Token reduction statistics
#[derive(Debug, Clone)]
pub struct TokenReductionStats {
    pub original_count: u32,
    pub reduced_count: u32,
    pub reduction_percentage: f32,
}

impl TokenReductionStats {
    pub fn new(original: u32, reduced: u32) -> Self {
        let reduction_percentage = if original > 0 {
            ((original - reduced) as f32 / original as f32) * 100.0
        } else {
            0.0
        };

        Self {
            original_count: original,
            reduced_count: reduced,
            reduction_percentage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_reduction_stats() {
        let stats = TokenReductionStats::new(1296, 291);
        assert_eq!(stats.original_count, 1296);
        assert_eq!(stats.reduced_count, 291);
        assert!((stats.reduction_percentage - 77.5).abs() < 0.1); // ShowUI paper example
    }

    #[tokio::test]
    async fn test_ui_token_selector_creation() {
        let config = TokenSelectionConfig::default_multi_monitor();
        let selector = UITokenSelector::new(config);
        assert!(selector.is_ok());
    }

    #[test]
    fn test_display_bounds_conversion() {
        let bounds = DisplayBounds {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };

        assert_eq!(bounds.width, 1920.0);
        assert_eq!(bounds.height, 1080.0);
    }
}
