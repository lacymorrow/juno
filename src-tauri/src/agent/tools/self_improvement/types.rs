//! Type definitions for the self-improvement system

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// Improvement strategy types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StrategyType {
    /// Self-Improving Coding Agent methodology
    SICA,
    /// Darwin Gödel Machine approach
    DarwinGodelMachine,
    /// Recursive self-improvement
    RecursiveSelfImprovement,
}

/// Focus areas for improvement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FocusArea {
    /// Tool usage optimization
    ToolUsage,
    /// Prompt effectiveness improvement
    PromptEffectiveness,
    /// Architecture optimization
    ArchitectureOptimization,
    /// Error handling enhancement
    ErrorHandling,
    /// Performance optimization
    Performance,
    /// Code quality improvement
    CodeQuality,
}

/// Benchmarking types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BenchmarkType {
    /// Accuracy benchmarking
    Accuracy,
    /// Performance benchmarking
    Performance,
    /// Reliability benchmarking
    Reliability,
    /// Cost benchmarking
    Cost,
    /// Innovation benchmarking
    Innovation,
}

/// Iteration status tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IterationStatus {
    /// Iteration is being planned
    Planning,
    /// Analysis is in progress
    Analyzing,
    /// Code generation is in progress
    Generating,
    /// Validation is in progress
    Validating,
    /// Benchmarking is in progress
    Benchmarking,
    /// Iteration completed successfully
    Completed,
    /// Iteration failed
    Failed,
    /// Iteration was cancelled
    Cancelled,
}

/// Improvement types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImprovementType {
    /// Performance optimization
    Performance,
    /// Bug fix
    BugFix,
    /// Code refactoring
    Refactoring,
    /// New feature
    NewFeature,
    /// Error handling improvement
    ErrorHandling,
    /// Documentation improvement
    Documentation,
}

/// Performance metrics for benchmarking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Average execution time (milliseconds)
    pub avg_execution_time: f64,
    /// Memory usage (MB)
    pub memory_usage: f64,
    /// CPU usage percentage
    pub cpu_usage: f64,
    /// Throughput (operations per second)
    pub throughput: f64,
}

/// Cost metrics for optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostMetrics {
    /// Computational cost score (0.0-1.0)
    pub computational_cost: f64,
    /// API call cost (if applicable)
    pub api_cost: f64,
    /// Resource utilization cost
    pub resource_cost: f64,
}

/// Reliability metrics for quality assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReliabilityMetrics {
    /// Success rate (0.0-1.0)
    pub success_rate: f64,
    /// Error rate (0.0-1.0)
    pub error_rate: f64,
    /// Mean time between failures (seconds)
    pub mtbf: f64,
    /// Recovery time (seconds)
    pub recovery_time: f64,
}

/// Comprehensive benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    /// Benchmark execution timestamp
    pub timestamp: DateTime<Utc>,
    /// Overall benchmark score (0.0-1.0)
    pub overall_score: f64,
    /// Individual benchmark scores
    pub scores: HashMap<BenchmarkType, f64>,
    /// Performance metrics
    pub performance_metrics: PerformanceMetrics,
    /// Cost metrics
    pub cost_metrics: CostMetrics,
    /// Reliability metrics
    pub reliability_metrics: ReliabilityMetrics,
}

/// Performance issue identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceIssue {
    /// Issue identifier
    pub id: String,
    /// Issue severity (0.0-1.0)
    pub severity: f64,
    /// Issue description
    pub description: String,
    /// Affected components
    pub affected_components: Vec<String>,
    /// Potential solutions
    pub potential_solutions: Vec<String>,
}

/// Improvement opportunity identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementOpportunity {
    /// Opportunity identifier
    pub id: String,
    /// Opportunity priority (0.0-1.0)
    pub priority: f64,
    /// Opportunity description
    pub description: String,
    /// Target focus area
    pub focus_area: FocusArea,
    /// Expected benefit score (0.0-1.0)
    pub expected_benefit: f64,
    /// Implementation complexity (0.0-1.0)
    pub complexity: f64,
}

/// Performance analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnalysis {
    /// Analysis timestamp
    pub timestamp: DateTime<Utc>,
    /// Overall system health score (0.0-1.0)
    pub health_score: f64,
    /// Identified issues
    pub issues: Vec<PerformanceIssue>,
    /// Improvement opportunities
    pub opportunities: Vec<ImprovementOpportunity>,
    /// Analysis recommendations
    pub recommendations: Vec<String>,
}

/// Code improvement specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeImprovement {
    /// Improvement identifier
    pub id: String,
    /// Target file path
    pub file_path: String,
    /// Improvement type
    pub improvement_type: ImprovementType,
    /// Original code
    pub original_code: String,
    /// Improved code
    pub improved_code: String,
    /// Improvement description
    pub description: String,
    /// Expected impact score (0.0-1.0)
    pub expected_impact: f64,
}

/// Resource usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU time used (seconds)
    pub cpu_time: f64,
    /// Memory usage (MB)
    pub memory_usage: f64,
    /// Disk I/O (MB)
    pub disk_io: f64,
    /// Network I/O (MB)
    pub network_io: f64,
}

/// Iteration metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationMetadata {
    /// Meta-agent used for this iteration
    pub meta_agent_id: Option<String>,
    /// Strategy used
    pub strategy: StrategyType,
    /// Focus areas targeted
    pub focus_areas: Vec<FocusArea>,
    /// Execution time (seconds)
    pub execution_time: f64,
    /// Resource usage during iteration
    pub resource_usage: ResourceUsage,
}

/// Complete improvement iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementIteration {
    /// Unique iteration identifier
    pub id: String,
    /// Iteration timestamp
    pub timestamp: DateTime<Utc>,
    /// Current status
    pub status: IterationStatus,
    /// Analysis results
    pub analysis: PerformanceAnalysis,
    /// Generated improvements
    pub improvements: Vec<CodeImprovement>,
    /// Benchmark results
    pub benchmark_results: Option<BenchmarkResults>,
    /// Overall utility score (0.0-1.0)
    pub utility_score: f64,
    /// Whether iteration was accepted
    pub accepted: bool,
    /// Iteration metadata
    pub metadata: IterationMetadata,
}
