//! Configuration module for UI-Guided Visual Token Selection
//!
//! Provides configuration options for controlling token selection behavior,
//! RGB analysis parameters, and multi-monitor optimization settings.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for UI-Guided Visual Token Selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSelectionConfig {
    /// RGB Analysis Configuration
    pub rgb_analysis: RGBAnalysisConfig,

    /// Token Reduction Configuration
    pub token_reduction: TokenReductionConfig,

    /// Multi-Monitor Optimization Configuration
    pub multi_monitor: MultiMonitorConfig,

    /// Performance Configuration
    pub performance: PerformanceConfig,
}

/// Configuration for RGB connected graph analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RGBAnalysisConfig {
    /// Color similarity threshold (0.0-1.0)
    /// Lower values = more strict color matching
    pub color_similarity_threshold: f32,

    /// Minimum patch size for analysis (pixels)
    pub min_patch_size: u32,

    /// Maximum patch size for analysis (pixels)
    pub max_patch_size: u32,

    /// Color variance threshold for grouping
    pub color_variance_threshold: f32,

    /// Enable edge detection for UI boundaries
    pub enable_edge_detection: bool,

    /// Edge detection sensitivity (0.0-1.0)
    pub edge_detection_sensitivity: f32,
}

/// Configuration for token reduction algorithms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenReductionConfig {
    /// Target reduction percentage (0.0-1.0)
    /// Based on ShowUI paper: 0.77 for 77% reduction in sparse areas
    pub target_reduction_percentage: f32,

    /// Minimum importance score to preserve token (0.0-1.0)
    pub min_importance_threshold: f32,

    /// Enable redundancy group optimization
    pub enable_redundancy_grouping: bool,

    /// Maximum tokens per redundancy group
    pub max_tokens_per_group: u32,

    /// Preserve interactive elements (buttons, links, etc.)
    pub preserve_interactive_elements: bool,

    /// Preserve text elements
    pub preserve_text_elements: bool,

    /// Background simplification level (0-3)
    /// 0 = No simplification, 3 = Maximum simplification
    pub background_simplification_level: u8,
}

/// Configuration for multi-monitor optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiMonitorConfig {
    /// Enable display-specific optimization
    pub enable_display_optimization: bool,

    /// Scale token reduction based on display resolution
    pub scale_by_resolution: bool,

    /// Different reduction targets per display type
    pub primary_display_reduction: f32,
    pub secondary_display_reduction: f32,

    /// Enable cross-display token correlation
    pub enable_cross_display_correlation: bool,

    /// Maximum displays to process simultaneously
    pub max_concurrent_displays: u32,

    /// Display processing timeout
    pub display_processing_timeout_ms: u64,
}

/// Configuration for performance optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Enable parallel processing for large images
    pub enable_parallel_processing: bool,

    /// Number of worker threads (0 = auto-detect)
    pub worker_threads: u32,

    /// Memory usage limit in MB
    pub memory_limit_mb: u64,

    /// Processing timeout in milliseconds
    pub processing_timeout_ms: u64,

    /// Enable performance metrics collection
    pub enable_metrics: bool,

    /// Metrics collection interval
    pub metrics_interval_ms: u64,

    /// Enable caching of analysis results
    pub enable_caching: bool,

    /// Cache size limit in MB
    pub cache_size_mb: u64,
}

impl TokenSelectionConfig {
    /// Creates a default configuration optimized for multi-monitor setups
    pub fn default_multi_monitor() -> Self {
        Self {
            rgb_analysis: RGBAnalysisConfig {
                color_similarity_threshold: 0.15, // Moderate similarity for UI elements
                min_patch_size: 8,                // Small patches for detail
                max_patch_size: 64,               // Larger patches for backgrounds
                color_variance_threshold: 0.1,    // Low variance for solid colors
                enable_edge_detection: true,      // Important for UI boundaries
                edge_detection_sensitivity: 0.3,  // Moderate sensitivity
            },
            token_reduction: TokenReductionConfig {
                target_reduction_percentage: 0.70, // 70% reduction target
                min_importance_threshold: 0.2,     // Keep moderately important tokens
                enable_redundancy_grouping: true,  // Essential for efficiency
                max_tokens_per_group: 3,           // Small groups for precision
                preserve_interactive_elements: true, // Critical for functionality
                preserve_text_elements: true,      // Important for readability
                background_simplification_level: 2, // High background simplification
            },
            multi_monitor: MultiMonitorConfig {
                enable_display_optimization: true,
                scale_by_resolution: true,
                primary_display_reduction: 0.65,   // Less aggressive on primary
                secondary_display_reduction: 0.75, // More aggressive on secondary
                enable_cross_display_correlation: true,
                max_concurrent_displays: 4,        // Support up to 4 displays
                display_processing_timeout_ms: 5000, // 5 second timeout
            },
            performance: PerformanceConfig {
                enable_parallel_processing: true,
                worker_threads: 0,                  // Auto-detect
                memory_limit_mb: 512,               // 512MB limit
                processing_timeout_ms: 10000,       // 10 second timeout
                enable_metrics: true,
                metrics_interval_ms: 1000,          // 1 second intervals
                enable_caching: true,
                cache_size_mb: 128,                 // 128MB cache
            },
        }
    }

    /// Creates a configuration optimized for single display setups
    pub fn default_single_monitor() -> Self {
        let mut config = Self::default_multi_monitor();

        // Adjust for single monitor
        config.multi_monitor.enable_display_optimization = false;
        config.multi_monitor.enable_cross_display_correlation = false;
        config.multi_monitor.max_concurrent_displays = 1;

        // More aggressive reduction for single display
        config.token_reduction.target_reduction_percentage = 0.75;

        config
    }

    /// Creates a high-performance configuration for powerful systems
    pub fn high_performance() -> Self {
        let mut config = Self::default_multi_monitor();

        // Increase parallel processing
        config.performance.worker_threads = 8;
        config.performance.memory_limit_mb = 1024;
        config.performance.processing_timeout_ms = 15000;

        // More detailed analysis
        config.rgb_analysis.min_patch_size = 4;
        config.rgb_analysis.color_similarity_threshold = 0.1;

        // Less aggressive reduction for quality
        config.token_reduction.target_reduction_percentage = 0.60;
        config.token_reduction.min_importance_threshold = 0.3;

        config
    }

    /// Creates a memory-efficient configuration for resource-constrained systems
    pub fn memory_efficient() -> Self {
        let mut config = Self::default_multi_monitor();

        // Reduce memory usage
        config.performance.memory_limit_mb = 256;
        config.performance.cache_size_mb = 64;
        config.performance.enable_parallel_processing = false;

        // Larger patches to reduce memory usage
        config.rgb_analysis.min_patch_size = 16;
        config.rgb_analysis.max_patch_size = 32;

        // More aggressive reduction
        config.token_reduction.target_reduction_percentage = 0.80;
        config.token_reduction.background_simplification_level = 3;

        config
    }

    /// Validates the configuration for consistency and reasonable values
    pub fn validate(&self) -> Result<(), String> {
        // Validate RGB analysis config
        if self.rgb_analysis.color_similarity_threshold < 0.0 || self.rgb_analysis.color_similarity_threshold > 1.0 {
            return Err("color_similarity_threshold must be between 0.0 and 1.0".to_string());
        }

        if self.rgb_analysis.min_patch_size >= self.rgb_analysis.max_patch_size {
            return Err("min_patch_size must be less than max_patch_size".to_string());
        }

        if self.rgb_analysis.edge_detection_sensitivity < 0.0 || self.rgb_analysis.edge_detection_sensitivity > 1.0 {
            return Err("edge_detection_sensitivity must be between 0.0 and 1.0".to_string());
        }

        // Validate token reduction config
        if self.token_reduction.target_reduction_percentage < 0.0 || self.token_reduction.target_reduction_percentage > 1.0 {
            return Err("target_reduction_percentage must be between 0.0 and 1.0".to_string());
        }

        if self.token_reduction.min_importance_threshold < 0.0 || self.token_reduction.min_importance_threshold > 1.0 {
            return Err("min_importance_threshold must be between 0.0 and 1.0".to_string());
        }

        if self.token_reduction.background_simplification_level > 3 {
            return Err("background_simplification_level must be between 0 and 3".to_string());
        }

        // Validate multi-monitor config
        if self.multi_monitor.primary_display_reduction < 0.0 || self.multi_monitor.primary_display_reduction > 1.0 {
            return Err("primary_display_reduction must be between 0.0 and 1.0".to_string());
        }

        if self.multi_monitor.secondary_display_reduction < 0.0 || self.multi_monitor.secondary_display_reduction > 1.0 {
            return Err("secondary_display_reduction must be between 0.0 and 1.0".to_string());
        }

        // Validate performance config
        if self.performance.memory_limit_mb == 0 {
            return Err("memory_limit_mb must be greater than 0".to_string());
        }

        if self.performance.processing_timeout_ms < 1000 {
            return Err("processing_timeout_ms should be at least 1000ms".to_string());
        }

        Ok(())
    }

    /// Gets the processing timeout as a Duration
    pub fn processing_timeout(&self) -> Duration {
        Duration::from_millis(self.performance.processing_timeout_ms)
    }

    /// Gets the display processing timeout as a Duration
    pub fn display_processing_timeout(&self) -> Duration {
        Duration::from_millis(self.multi_monitor.display_processing_timeout_ms)
    }

    /// Gets the metrics collection interval as a Duration
    pub fn metrics_interval(&self) -> Duration {
        Duration::from_millis(self.performance.metrics_interval_ms)
    }
}

impl Default for TokenSelectionConfig {
    fn default() -> Self {
        Self::default_multi_monitor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_multi_monitor_config() {
        let config = TokenSelectionConfig::default_multi_monitor();
        assert!(config.validate().is_ok());
        assert!(config.multi_monitor.enable_display_optimization);
        assert_eq!(config.token_reduction.target_reduction_percentage, 0.70);
    }

    #[test]
    fn test_single_monitor_config() {
        let config = TokenSelectionConfig::default_single_monitor();
        assert!(config.validate().is_ok());
        assert!(!config.multi_monitor.enable_display_optimization);
        assert_eq!(config.multi_monitor.max_concurrent_displays, 1);
    }

    #[test]
    fn test_high_performance_config() {
        let config = TokenSelectionConfig::high_performance();
        assert!(config.validate().is_ok());
        assert_eq!(config.performance.worker_threads, 8);
        assert_eq!(config.performance.memory_limit_mb, 1024);
    }

    #[test]
    fn test_memory_efficient_config() {
        let config = TokenSelectionConfig::memory_efficient();
        assert!(config.validate().is_ok());
        assert_eq!(config.performance.memory_limit_mb, 256);
        assert!(!config.performance.enable_parallel_processing);
    }

    #[test]
    fn test_config_validation() {
        let mut config = TokenSelectionConfig::default();

        // Test invalid color similarity threshold
        config.rgb_analysis.color_similarity_threshold = 1.5;
        assert!(config.validate().is_err());

        // Reset and test invalid patch sizes
        config = TokenSelectionConfig::default();
        config.rgb_analysis.min_patch_size = 100;
        config.rgb_analysis.max_patch_size = 50;
        assert!(config.validate().is_err());

        // Reset and test invalid reduction percentage
        config = TokenSelectionConfig::default();
        config.token_reduction.target_reduction_percentage = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_duration_conversions() {
        let config = TokenSelectionConfig::default();

        assert_eq!(config.processing_timeout().as_millis(), config.performance.processing_timeout_ms as u128);
        assert_eq!(config.display_processing_timeout().as_millis(), config.multi_monitor.display_processing_timeout_ms as u128);
        assert_eq!(config.metrics_interval().as_millis(), config.performance.metrics_interval_ms as u128);
    }
}
