//! Performance tracking module for UI-Guided Visual Token Selection
//!
//! Provides comprehensive performance monitoring, metrics collection,
//! and optimization insights for token selection operations.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Performance metrics for UI-Guided Visual Token Selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Total number of screenshots processed
    pub total_screenshots: u64,

    /// Total processing time across all operations
    pub total_processing_time_ms: u64,

    /// Average processing time per screenshot
    pub avg_processing_time_ms: f64,

    /// Token reduction statistics
    pub token_stats: TokenReductionMetrics,

    /// Memory usage statistics
    pub memory_stats: MemoryUsageMetrics,

    /// Multi-monitor specific metrics
    pub multi_monitor_stats: MultiMonitorMetrics,

    /// Performance trends over time
    pub performance_trends: PerformanceTrends,

    /// Last updated timestamp
    pub last_updated: u64,
}

/// Token reduction performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenReductionMetrics {
    /// Total original tokens processed
    pub total_original_tokens: u64,

    /// Total reduced tokens produced
    pub total_reduced_tokens: u64,

    /// Average reduction percentage
    pub avg_reduction_percentage: f32,

    /// Best reduction percentage achieved
    pub best_reduction_percentage: f32,

    /// Worst reduction percentage achieved
    pub worst_reduction_percentage: f32,

    /// Computational cost savings (estimated)
    pub estimated_cost_savings_percentage: f32,
}

/// Memory usage performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsageMetrics {
    /// Peak memory usage in MB
    pub peak_memory_mb: f64,

    /// Average memory usage in MB
    pub avg_memory_mb: f64,

    /// Current memory usage in MB
    pub current_memory_mb: f64,

    /// Memory efficiency score (0.0-1.0)
    pub memory_efficiency_score: f32,
}

/// Multi-monitor specific performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiMonitorMetrics {
    /// Number of different display configurations processed
    pub display_configurations_processed: u32,

    /// Average performance gain from multi-monitor optimization
    pub avg_multi_monitor_gain: f32,

    /// Performance by display count
    pub performance_by_display_count: Vec<DisplayCountPerformance>,

    /// Cross-display correlation effectiveness
    pub cross_display_correlation_effectiveness: f32,
}

/// Performance metrics for specific display counts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayCountPerformance {
    pub display_count: u32,
    pub avg_reduction_percentage: f32,
    pub avg_processing_time_ms: f64,
    pub sample_count: u32,
}

/// Performance trends over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTrends {
    /// Recent processing times (last 100 operations)
    pub recent_processing_times: Vec<u64>,

    /// Recent reduction percentages (last 100 operations)
    pub recent_reduction_percentages: Vec<f32>,

    /// Trend direction (-1.0 to 1.0, negative = getting worse, positive = getting better)
    pub processing_time_trend: f32,
    pub reduction_percentage_trend: f32,
}

/// Performance tracker for UI-Guided Visual Token Selection
pub struct PerformanceTracker {
    data: Arc<Mutex<PerformanceMetrics>>,
}

impl PerformanceTracker {
    /// Creates a new performance tracker
    pub fn new() -> Self {
        let initial_metrics = PerformanceMetrics {
            total_screenshots: 0,
            total_processing_time_ms: 0,
            avg_processing_time_ms: 0.0,
            token_stats: TokenReductionMetrics {
                total_original_tokens: 0,
                total_reduced_tokens: 0,
                avg_reduction_percentage: 0.0,
                best_reduction_percentage: 0.0,
                worst_reduction_percentage: 100.0,
                estimated_cost_savings_percentage: 0.0,
            },
            memory_stats: MemoryUsageMetrics {
                peak_memory_mb: 0.0,
                avg_memory_mb: 0.0,
                current_memory_mb: 0.0,
                memory_efficiency_score: 1.0,
            },
            multi_monitor_stats: MultiMonitorMetrics {
                display_configurations_processed: 0,
                avg_multi_monitor_gain: 0.0,
                performance_by_display_count: Vec::new(),
                cross_display_correlation_effectiveness: 0.0,
            },
            performance_trends: PerformanceTrends {
                recent_processing_times: Vec::new(),
                recent_reduction_percentages: Vec::new(),
                processing_time_trend: 0.0,
                reduction_percentage_trend: 0.0,
            },
            last_updated: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        Self {
            data: Arc::new(Mutex::new(initial_metrics)),
        }
    }

    /// Records a processing operation
    pub fn record_processing(
        &self,
        original_tokens: u32,
        reduced_tokens: u32,
        processing_time: Duration,
    ) -> Result<(), String> {
        let mut metrics = self.data.lock()
            .map_err(|e| format!("Failed to acquire performance data lock: {}", e))?;

        let processing_time_ms = processing_time.as_millis() as u64;
        let reduction_percentage = if original_tokens > 0 {
            ((original_tokens - reduced_tokens) as f32 / original_tokens as f32) * 100.0
        } else {
            0.0
        };

        // Update basic metrics
        metrics.total_screenshots += 1;
        metrics.total_processing_time_ms += processing_time_ms;
        metrics.avg_processing_time_ms = metrics.total_processing_time_ms as f64 / metrics.total_screenshots as f64;

        // Update token statistics
        metrics.token_stats.total_original_tokens += original_tokens as u64;
        metrics.token_stats.total_reduced_tokens += reduced_tokens as u64;
        metrics.token_stats.avg_reduction_percentage =
            ((metrics.token_stats.total_original_tokens - metrics.token_stats.total_reduced_tokens) as f32
             / metrics.token_stats.total_original_tokens as f32) * 100.0;

        // Update best/worst reduction percentages
        if reduction_percentage > metrics.token_stats.best_reduction_percentage {
            metrics.token_stats.best_reduction_percentage = reduction_percentage;
        }
        if reduction_percentage < metrics.token_stats.worst_reduction_percentage {
            metrics.token_stats.worst_reduction_percentage = reduction_percentage;
        }

        // Estimate cost savings (based on ShowUI paper: 33% computational cost reduction)
        metrics.token_stats.estimated_cost_savings_percentage =
            metrics.token_stats.avg_reduction_percentage * 0.33;

        // Update trends
        metrics.performance_trends.recent_processing_times.push(processing_time_ms);
        metrics.performance_trends.recent_reduction_percentages.push(reduction_percentage);

        // Keep only last 100 entries
        if metrics.performance_trends.recent_processing_times.len() > 100 {
            metrics.performance_trends.recent_processing_times.remove(0);
        }
        if metrics.performance_trends.recent_reduction_percentages.len() > 100 {
            metrics.performance_trends.recent_reduction_percentages.remove(0);
        }

        // Update timestamp
        metrics.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        debug!(
            "Performance recorded: {}ms, {:.1}% reduction ({} -> {} tokens)",
            processing_time_ms, reduction_percentage, original_tokens, reduced_tokens
        );

        Ok(())
    }

    /// Gets current performance metrics
    pub fn get_metrics(&self) -> PerformanceMetrics {
        match self.data.lock() {
            Ok(metrics) => metrics.clone(),
            Err(e) => {
                warn!("Failed to acquire performance data lock for metrics: {}", e);
                // Return default metrics as fallback
                PerformanceMetrics {
                    total_screenshots: 0,
                    total_processing_time_ms: 0,
                    avg_processing_time_ms: 0.0,
                    token_stats: TokenReductionMetrics {
                        total_original_tokens: 0,
                        total_reduced_tokens: 0,
                        avg_reduction_percentage: 0.0,
                        best_reduction_percentage: 0.0,
                        worst_reduction_percentage: 100.0,
                        estimated_cost_savings_percentage: 0.0,
                    },
                    memory_stats: MemoryUsageMetrics {
                        peak_memory_mb: 0.0,
                        avg_memory_mb: 0.0,
                        current_memory_mb: 0.0,
                        memory_efficiency_score: 1.0,
                    },
                    multi_monitor_stats: MultiMonitorMetrics {
                        display_configurations_processed: 0,
                        avg_multi_monitor_gain: 0.0,
                        performance_by_display_count: Vec::new(),
                        cross_display_correlation_effectiveness: 0.0,
                    },
                    performance_trends: PerformanceTrends {
                        recent_processing_times: Vec::new(),
                        recent_reduction_percentages: Vec::new(),
                        processing_time_trend: 0.0,
                        reduction_percentage_trend: 0.0,
                    },
                    last_updated: 0,
                }
            }
        }
    }

    /// Resets all performance metrics
    pub fn reset(&self) {
        match self.data.lock() {
            Ok(mut metrics) => {
                *metrics = PerformanceMetrics {
                    total_screenshots: 0,
                    total_processing_time_ms: 0,
                    avg_processing_time_ms: 0.0,
                    token_stats: TokenReductionMetrics {
                        total_original_tokens: 0,
                        total_reduced_tokens: 0,
                        avg_reduction_percentage: 0.0,
                        best_reduction_percentage: 0.0,
                        worst_reduction_percentage: 100.0,
                        estimated_cost_savings_percentage: 0.0,
                    },
                    memory_stats: MemoryUsageMetrics {
                        peak_memory_mb: 0.0,
                        avg_memory_mb: 0.0,
                        current_memory_mb: 0.0,
                        memory_efficiency_score: 1.0,
                    },
                    multi_monitor_stats: MultiMonitorMetrics {
                        display_configurations_processed: 0,
                        avg_multi_monitor_gain: 0.0,
                        performance_by_display_count: Vec::new(),
                        cross_display_correlation_effectiveness: 0.0,
                    },
                    performance_trends: PerformanceTrends {
                        recent_processing_times: Vec::new(),
                        recent_reduction_percentages: Vec::new(),
                        processing_time_trend: 0.0,
                        reduction_percentage_trend: 0.0,
                    },
                    last_updated: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };
                info!("Performance metrics reset");
            }
            Err(e) => {
                warn!("Failed to reset performance metrics: {}", e);
            }
        }
    }
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_performance_tracker_creation() {
        let tracker = PerformanceTracker::new();
        let metrics = tracker.get_metrics();
        assert_eq!(metrics.total_screenshots, 0);
        assert_eq!(metrics.avg_processing_time_ms, 0.0);
    }

    #[test]
    fn test_record_processing() {
        let tracker = PerformanceTracker::new();

        let result = tracker.record_processing(1296, 291, Duration::from_millis(100));
        assert!(result.is_ok());

        let metrics = tracker.get_metrics();
        assert_eq!(metrics.total_screenshots, 1);
        assert_eq!(metrics.avg_processing_time_ms, 100.0);
        assert!((metrics.token_stats.avg_reduction_percentage - 77.5).abs() < 0.1);
    }

    #[test]
    fn test_multiple_operations() {
        let tracker = PerformanceTracker::new();

        // Record multiple operations
        tracker.record_processing(1000, 300, Duration::from_millis(50)).unwrap();
        tracker.record_processing(2000, 500, Duration::from_millis(150)).unwrap();

        let metrics = tracker.get_metrics();
        assert_eq!(metrics.total_screenshots, 2);
        assert_eq!(metrics.avg_processing_time_ms, 100.0); // (50 + 150) / 2
        assert_eq!(metrics.token_stats.total_original_tokens, 3000);
        assert_eq!(metrics.token_stats.total_reduced_tokens, 800);
    }

    #[test]
    fn test_performance_reset() {
        let tracker = PerformanceTracker::new();

        tracker.record_processing(1000, 300, Duration::from_millis(100)).unwrap();
        assert_eq!(tracker.get_metrics().total_screenshots, 1);

        tracker.reset();
        assert_eq!(tracker.get_metrics().total_screenshots, 0);
    }
}
