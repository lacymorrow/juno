/// Performance testing utilities for Juno AI Computer Use Agent
/// 
/// This module provides utilities for:
/// - Measuring execution times
/// - Memory usage tracking
/// - Performance benchmarking
/// - Assertion helpers for performance tests

use std::time::{Duration, Instant};
use std::collections::HashMap;

/// Performance metrics collector
pub struct PerformanceMetrics {
    start_time: Instant,
    measurements: HashMap<String, Vec<Duration>>,
    memory_snapshots: Vec<MemorySnapshot>,
}

#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    pub timestamp: Instant,
    pub allocated_bytes: usize,
    pub resident_bytes: usize,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            measurements: HashMap::new(),
            memory_snapshots: Vec::new(),
        }
    }

    /// Start measuring a named operation
    pub fn start_measurement(&self, name: &str) -> PerformanceMeasurement {
        PerformanceMeasurement::new(name.to_string())
    }

    /// Record a measurement
    pub fn record(&mut self, name: String, duration: Duration) {
        self.measurements.entry(name).or_insert_with(Vec::new).push(duration);
    }

    /// Take a memory snapshot
    pub fn snapshot_memory(&mut self) {
        let snapshot = MemorySnapshot {
            timestamp: Instant::now(),
            allocated_bytes: get_allocated_memory(),
            resident_bytes: get_resident_memory(),
        };
        self.memory_snapshots.push(snapshot);
    }

    /// Get average duration for a measurement
    pub fn average_duration(&self, name: &str) -> Option<Duration> {
        self.measurements.get(name).map(|durations| {
            let total: Duration = durations.iter().sum();
            total / durations.len() as u32
        })
    }

    /// Get percentile for a measurement
    pub fn percentile(&self, name: &str, percentile: f64) -> Option<Duration> {
        self.measurements.get(name).and_then(|durations| {
            if durations.is_empty() {
                return None;
            }
            
            let mut sorted = durations.clone();
            sorted.sort();
            
            let index = ((percentile / 100.0) * (sorted.len() - 1) as f64).round() as usize;
            sorted.get(index).copied()
        })
    }

    /// Get memory usage statistics
    pub fn memory_stats(&self) -> MemoryStats {
        if self.memory_snapshots.is_empty() {
            return MemoryStats::default();
        }

        let allocated_bytes: Vec<usize> = self.memory_snapshots.iter()
            .map(|s| s.allocated_bytes)
            .collect();
        
        let resident_bytes: Vec<usize> = self.memory_snapshots.iter()
            .map(|s| s.resident_bytes)
            .collect();

        MemoryStats {
            min_allocated: *allocated_bytes.iter().min().unwrap(),
            max_allocated: *allocated_bytes.iter().max().unwrap(),
            avg_allocated: allocated_bytes.iter().sum::<usize>() / allocated_bytes.len(),
            min_resident: *resident_bytes.iter().min().unwrap(),
            max_resident: *resident_bytes.iter().max().unwrap(),
            avg_resident: resident_bytes.iter().sum::<usize>() / resident_bytes.len(),
            peak_usage: *allocated_bytes.iter().max().unwrap(),
        }
    }

    /// Print performance summary
    pub fn print_summary(&self) {
        println!("\n=== Performance Summary ===");
        println!("Total test duration: {:?}", self.start_time.elapsed());
        
        for (name, durations) in &self.measurements {
            if let Some(avg) = self.average_duration(name) {
                let min = durations.iter().min().unwrap();
                let max = durations.iter().max().unwrap();
                let p95 = self.percentile(name, 95.0).unwrap_or(*max);
                
                println!("\n{}: {} samples", name, durations.len());
                println!("  Average: {:?}", avg);
                println!("  Min: {:?}", min);
                println!("  Max: {:?}", max);
                println!("  95th percentile: {:?}", p95);
            }
        }

        if !self.memory_snapshots.is_empty() {
            let stats = self.memory_stats();
            println!("\nMemory Usage:");
            println!("  Peak allocated: {} MB", stats.peak_usage / 1024 / 1024);
            println!("  Average allocated: {} MB", stats.avg_allocated / 1024 / 1024);
            println!("  Average resident: {} MB", stats.avg_resident / 1024 / 1024);
        }
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub min_allocated: usize,
    pub max_allocated: usize,
    pub avg_allocated: usize,
    pub min_resident: usize,
    pub max_resident: usize,
    pub avg_resident: usize,
    pub peak_usage: usize,
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            min_allocated: 0,
            max_allocated: 0,
            avg_allocated: 0,
            min_resident: 0,
            max_resident: 0,
            avg_resident: 0,
            peak_usage: 0,
        }
    }
}

/// Individual performance measurement
pub struct PerformanceMeasurement {
    name: String,
    start_time: Instant,
}

impl PerformanceMeasurement {
    fn new(name: String) -> Self {
        Self {
            name,
            start_time: Instant::now(),
        }
    }

    /// Finish the measurement and return the duration
    pub fn finish(self) -> (String, Duration) {
        (self.name, self.start_time.elapsed())
    }
}

/// Performance assertion helpers
pub struct PerformanceAssertions;

impl PerformanceAssertions {
    /// Assert that a duration is within acceptable bounds
    pub fn assert_duration_within(duration: Duration, max_duration: Duration, operation: &str) {
        assert!(
            duration <= max_duration,
            "Performance assertion failed for '{}': took {:?}, expected <= {:?}",
            operation, duration, max_duration
        );
    }

    /// Assert that memory usage is within bounds
    pub fn assert_memory_within(current_bytes: usize, max_bytes: usize, context: &str) {
        assert!(
            current_bytes <= max_bytes,
            "Memory assertion failed for '{}': using {} bytes, expected <= {} bytes",
            context, current_bytes, max_bytes
        );
    }

    /// Assert that average performance meets expectations
    pub fn assert_average_performance(
        metrics: &PerformanceMetrics,
        operation: &str,
        max_avg_duration: Duration,
    ) {
        if let Some(avg_duration) = metrics.average_duration(operation) {
            Self::assert_duration_within(avg_duration, max_avg_duration, &format!("average {}", operation));
        } else {
            panic!("No measurements found for operation: {}", operation);
        }
    }

    /// Assert that 95th percentile meets expectations
    pub fn assert_p95_performance(
        metrics: &PerformanceMetrics,
        operation: &str,
        max_p95_duration: Duration,
    ) {
        if let Some(p95_duration) = metrics.percentile(operation, 95.0) {
            Self::assert_duration_within(p95_duration, max_p95_duration, &format!("p95 {}", operation));
        } else {
            panic!("No measurements found for operation: {}", operation);
        }
    }

    /// Assert that memory usage doesn't exceed limits
    pub fn assert_memory_limits(metrics: &PerformanceMetrics, max_peak_mb: usize) {
        let stats = metrics.memory_stats();
        let peak_mb = stats.peak_usage / 1024 / 1024;
        
        assert!(
            peak_mb <= max_peak_mb,
            "Memory usage exceeded limit: peak {} MB, limit {} MB",
            peak_mb, max_peak_mb
        );
    }
}

/// Simplified memory tracking (mock implementation for testing)
fn get_allocated_memory() -> usize {
    // In a real implementation, this would use a memory profiling library
    // For testing, we'll return a mock value
    std::thread_local! {
        static MOCK_MEMORY: std::cell::Cell<usize> = const { std::cell::Cell::new(1024 * 1024) }; // 1MB baseline
    }
    
    MOCK_MEMORY.with(|m| {
        let current = m.get();
        // Simulate some memory variation
        let variation = (std::ptr::addr_of!(current) as usize % 1024) * 1024;
        current + variation
    })
}

fn get_resident_memory() -> usize {
    // Mock resident memory as slightly higher than allocated
    get_allocated_memory() + (512 * 1024) // +512KB
}

/// Macro for easy performance measurement
#[macro_export]
macro_rules! measure_performance {
    ($metrics:expr, $name:expr, $code:block) => {{
        let measurement = $metrics.start_measurement($name);
        let result = $code;
        let (name, duration) = measurement.finish();
        $metrics.record(name, duration);
        result
    }};
}

/// Performance test runner for agent operations
pub struct AgentPerformanceTest {
    metrics: PerformanceMetrics,
    test_name: String,
}

impl AgentPerformanceTest {
    pub fn new(test_name: &str) -> Self {
        Self {
            metrics: PerformanceMetrics::new(),
            test_name: test_name.to_string(),
        }
    }

    pub fn metrics_mut(&mut self) -> &mut PerformanceMetrics {
        &mut self.metrics
    }

    pub fn metrics(&self) -> &PerformanceMetrics {
        &self.metrics
    }

    /// Run agent query performance test
    pub async fn test_agent_query_performance<F, Fut>(&mut self, query: &str, agent_fn: F)
    where
        F: FnOnce(&str) -> Fut,
        Fut: std::future::Future<Output = Result<String, String>>,
    {
        self.metrics.snapshot_memory();
        
        let measurement = self.metrics.start_measurement("agent_query");
        let result = agent_fn(query).await;
        let (name, duration) = measurement.finish();
        self.metrics.record(name, duration);
        
        self.metrics.snapshot_memory();
        
        match result {
            Ok(_) => println!("Agent query completed in {:?}", duration),
            Err(e) => println!("Agent query failed in {:?}: {}", duration, e),
        }
    }

    /// Run tool execution performance test
    pub async fn test_tool_performance<F, Fut>(&mut self, tool_name: &str, tool_fn: F)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        self.metrics.snapshot_memory();
        
        let measurement = self.metrics.start_measurement(&format!("tool_{}", tool_name));
        let result = tool_fn().await;
        let (name, duration) = measurement.finish();
        self.metrics.record(name, duration);
        
        self.metrics.snapshot_memory();
        
        match result {
            Ok(_) => println!("Tool '{}' executed in {:?}", tool_name, duration),
            Err(e) => println!("Tool '{}' failed in {:?}: {}", tool_name, duration, e),
        }
    }

    /// Finish the test and print results
    pub fn finish(self) -> PerformanceTestResults {
        self.metrics.print_summary();
        
        PerformanceTestResults {
            test_name: self.test_name,
            metrics: self.metrics,
        }
    }
}

pub struct PerformanceTestResults {
    pub test_name: String,
    pub metrics: PerformanceMetrics,
}

impl PerformanceTestResults {
    /// Validate that performance meets requirements
    pub fn validate_requirements(&self, requirements: &PerformanceRequirements) -> Result<(), String> {
        // Check agent response time
        if let Some(max_agent_time) = requirements.max_agent_response_time {
            if let Some(avg_time) = self.metrics.average_duration("agent_query") {
                if avg_time > max_agent_time {
                    return Err(format!(
                        "Agent response time too slow: {:?} > {:?}",
                        avg_time, max_agent_time
                    ));
                }
            }
        }

        // Check tool execution time
        if let Some(max_tool_time) = requirements.max_tool_execution_time {
            for (name, durations) in &self.metrics.measurements {
                if name.starts_with("tool_") {
                    let avg_time = durations.iter().sum::<Duration>() / durations.len() as u32;
                    if avg_time > max_tool_time {
                        return Err(format!(
                            "Tool execution too slow for {}: {:?} > {:?}",
                            name, avg_time, max_tool_time
                        ));
                    }
                }
            }
        }

        // Check memory usage
        if let Some(max_memory_mb) = requirements.max_memory_usage_mb {
            let stats = self.metrics.memory_stats();
            let peak_mb = stats.peak_usage / 1024 / 1024;
            if peak_mb > max_memory_mb {
                return Err(format!(
                    "Memory usage too high: {} MB > {} MB",
                    peak_mb, max_memory_mb
                ));
            }
        }

        Ok(())
    }
}

pub struct PerformanceRequirements {
    pub max_agent_response_time: Option<Duration>,
    pub max_tool_execution_time: Option<Duration>,
    pub max_memory_usage_mb: Option<usize>,
}

impl Default for PerformanceRequirements {
    fn default() -> Self {
        Self {
            max_agent_response_time: Some(Duration::from_secs(5)),
            max_tool_execution_time: Some(Duration::from_secs(10)),
            max_memory_usage_mb: Some(500),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[test]
    fn test_performance_metrics() {
        let mut metrics = PerformanceMetrics::new();
        
        metrics.record("test_op".to_string(), Duration::from_millis(100));
        metrics.record("test_op".to_string(), Duration::from_millis(200));
        metrics.record("test_op".to_string(), Duration::from_millis(150));
        
        let avg = metrics.average_duration("test_op").unwrap();
        assert_eq!(avg, Duration::from_millis(150));
        
        let p95 = metrics.percentile("test_op", 95.0).unwrap();
        assert!(p95 >= Duration::from_millis(150));
    }

    #[test]
    fn test_memory_snapshots() {
        let mut metrics = PerformanceMetrics::new();
        
        metrics.snapshot_memory();
        metrics.snapshot_memory();
        
        let stats = metrics.memory_stats();
        assert!(stats.avg_allocated > 0);
        assert!(stats.avg_resident > 0);
    }

    #[tokio::test]
    async fn test_agent_performance_test() {
        let mut test = AgentPerformanceTest::new("test");
        
        test.test_agent_query_performance("test query", |_query| async {
            sleep(Duration::from_millis(10)).await;
            Ok("response".to_string())
        }).await;
        
        let results = test.finish();
        assert!(results.metrics.average_duration("agent_query").is_some());
    }

    #[tokio::test]
    async fn test_tool_performance_test() {
        let mut test = AgentPerformanceTest::new("test");
        
        test.test_tool_performance("screenshot", || async {
            sleep(Duration::from_millis(50)).await;
            Ok(())
        }).await;
        
        let results = test.finish();
        assert!(results.metrics.average_duration("tool_screenshot").is_some());
    }

    #[test]
    fn test_performance_assertions() {
        PerformanceAssertions::assert_duration_within(
            Duration::from_millis(100),
            Duration::from_millis(200),
            "test operation"
        );
        
        PerformanceAssertions::assert_memory_within(
            1024 * 1024, // 1MB
            2 * 1024 * 1024, // 2MB limit
            "test context"
        );
    }

    #[test]
    #[should_panic(expected = "Performance assertion failed")]
    fn test_performance_assertion_failure() {
        PerformanceAssertions::assert_duration_within(
            Duration::from_millis(300),
            Duration::from_millis(200),
            "slow operation"
        );
    }

    #[test]
    fn test_performance_requirements() {
        let mut metrics = PerformanceMetrics::new();
        metrics.record("agent_query".to_string(), Duration::from_millis(100));
        
        let results = PerformanceTestResults {
            test_name: "test".to_string(),
            metrics,
        };
        
        let requirements = PerformanceRequirements::default();
        assert!(results.validate_requirements(&requirements).is_ok());
        
        let strict_requirements = PerformanceRequirements {
            max_agent_response_time: Some(Duration::from_millis(50)),
            ..Default::default()
        };
        assert!(results.validate_requirements(&strict_requirements).is_err());
    }
}