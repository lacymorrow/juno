//! Configuration types for the self-improvement system

use serde::{Deserialize, Serialize};
use std::time::Duration;
use super::types::{BenchmarkType, FocusArea, StrategyType};

/// Configuration for the self-improvement system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfImprovementConfig {
    /// Enable development mode features
    pub development_mode: bool,
    /// Maximum number of iterations to store in archive
    pub max_archive_size: usize,
    /// Performance thresholds for improvement acceptance
    pub performance_thresholds: PerformanceThresholds,
    /// Safety constraints for code modifications
    pub safety_constraints: SafetyConstraints,
    /// Benchmarking configuration
    pub benchmark_config: BenchmarkConfig,
    /// Improvement strategy configuration
    pub improvement_strategy: ImprovementStrategy,
}

impl Default for SelfImprovementConfig {
    fn default() -> Self {
        Self {
            development_mode: cfg!(debug_assertions),
            max_archive_size: 100,
            performance_thresholds: PerformanceThresholds::default(),
            safety_constraints: SafetyConstraints::default(),
            benchmark_config: BenchmarkConfig::default(),
            improvement_strategy: ImprovementStrategy::default(),
        }
    }
}

/// Performance thresholds for improvement acceptance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// Minimum accuracy improvement required (0.0-1.0)
    pub min_accuracy_improvement: f64,
    /// Minimum performance improvement required (0.0-1.0)
    pub min_performance_improvement: f64,
    /// Maximum cost increase allowed (0.0-1.0)
    pub max_cost_increase: f64,
    /// Minimum reliability score required (0.0-1.0)
    pub min_reliability_score: f64,
    /// Minimum overall utility score for acceptance (0.0-1.0)
    pub min_utility_score: f64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            min_accuracy_improvement: 0.05,    // 5% minimum improvement
            min_performance_improvement: 0.03, // 3% minimum improvement
            max_cost_increase: 0.10,           // 10% maximum cost increase
            min_reliability_score: 0.80,       // 80% minimum reliability
            min_utility_score: 0.70,           // 70% minimum utility score
        }
    }
}

/// Safety constraints for code modifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConstraints {
    /// Enable comprehensive sandboxing
    pub enable_sandboxing: bool,
    /// Enable human oversight for critical changes
    pub require_human_approval: bool,
    /// Enable automatic backup before changes
    pub enable_backup: bool,
    /// Protected file patterns (regex patterns)
    pub protected_files: Vec<String>,
    /// Maximum file size for modifications (bytes)
    pub max_file_size: usize,
    /// Maximum number of files to modify per iteration
    pub max_files_per_iteration: usize,
}

impl Default for SafetyConstraints {
    fn default() -> Self {
        Self {
            enable_sandboxing: true,
            require_human_approval: true,
            enable_backup: true,
            protected_files: vec![
                r".*/(lib|main)\.rs$".to_string(),
                r".*/Cargo\.toml$".to_string(),
                r".*/package\.json$".to_string(),
                r".*\.env$".to_string(),
            ],
            max_file_size: 1024 * 1024, // 1MB
            max_files_per_iteration: 5,
        }
    }
}

/// Benchmarking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// Enable comprehensive benchmarking
    pub enable_benchmarking: bool,
    /// Benchmark types to run
    pub benchmark_types: Vec<BenchmarkType>,
    /// Performance configuration
    pub performance_config: PerformanceConfig,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            enable_benchmarking: true,
            benchmark_types: vec![
                BenchmarkType::Accuracy,
                BenchmarkType::Performance,
                BenchmarkType::Reliability,
                BenchmarkType::Cost,
            ],
            performance_config: PerformanceConfig::default(),
        }
    }
}

/// Performance configuration for benchmarking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Number of benchmark iterations
    pub iterations: usize,
    /// Timeout for each benchmark (seconds)
    pub timeout_seconds: u64,
    /// Enable parallel execution
    pub enable_parallel: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            iterations: 10,
            timeout_seconds: 300, // 5 minutes
            enable_parallel: true,
        }
    }
}

/// Improvement strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementStrategy {
    /// Strategy type
    pub strategy_type: StrategyType,
    /// Focus areas for improvement
    pub focus_areas: Vec<FocusArea>,
    /// Meta-agent selection criteria
    pub meta_agent_criteria: MetaAgentCriteria,
}

impl Default for ImprovementStrategy {
    fn default() -> Self {
        Self {
            strategy_type: StrategyType::SICA,
            focus_areas: vec![
                FocusArea::ToolUsage,
                FocusArea::PromptEffectiveness,
                FocusArea::ArchitectureOptimization,
                FocusArea::ErrorHandling,
            ],
            meta_agent_criteria: MetaAgentCriteria::default(),
        }
    }
}

/// Meta-agent selection criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaAgentCriteria {
    /// Weight for accuracy in selection (0.0-1.0)
    pub accuracy_weight: f64,
    /// Weight for performance in selection (0.0-1.0)
    pub performance_weight: f64,
    /// Weight for cost efficiency in selection (0.0-1.0)
    pub cost_weight: f64,
    /// Weight for reliability in selection (0.0-1.0)
    pub reliability_weight: f64,
    /// Weight for innovation in selection (0.0-1.0)
    pub innovation_weight: f64,
}

impl Default for MetaAgentCriteria {
    fn default() -> Self {
        Self {
            accuracy_weight: 0.25,
            performance_weight: 0.25,
            cost_weight: 0.15,
            reliability_weight: 0.25,
            innovation_weight: 0.10,
        }
    }
}
