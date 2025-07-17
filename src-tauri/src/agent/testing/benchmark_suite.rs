//! Performance Benchmarking Suite
//!
//! TARS Phase 3.6.4: Performance benchmarking and metrics
//!
//! Comprehensive benchmarking system for evaluating the performance
//! optimizations implemented in the event-driven memory system.

use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use tokio::sync::Mutex as TokioMutex;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::test_utilities::{TestCase, TestConfig, TestResult, TestMetrics, TestUtilities, MemoryMonitor};
use crate::agent::traits::MemoryManager;
use crate::agent::memory::performance::{PerformanceMetrics, PerformanceConfig, ObjectPool, SmartCache};
use crate::agent::events::{OptimizedEventBus, OptimizedEventBusConfig, JunoAgentEvent};

/// Benchmark configuration for different performance scenarios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// Name of the benchmark
    pub name: String,
    /// Description of what is being tested
    pub description: String,
    /// Number of iterations to run
    pub iterations: usize,
    /// Warmup iterations (not counted in results)
    pub warmup_iterations: usize,
    /// Test duration limit
    pub max_duration: Duration,
    /// Enable detailed metrics collection
    pub collect_detailed_metrics: bool,
    /// Performance configuration to test
    pub performance_config: PerformanceConfig,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            name: "DefaultBenchmark".to_string(),
            description: "Default benchmark configuration".to_string(),
            iterations: 1000,
            warmup_iterations: 100,
            max_duration: Duration::from_secs(60),
            collect_detailed_metrics: true,
            performance_config: PerformanceConfig::default(),
        }
    }
}

impl BenchmarkConfig {
    /// Create a high-throughput benchmark configuration
    pub fn high_throughput() -> Self {
        Self {
            name: "HighThroughput".to_string(),
            description: "High-throughput event processing benchmark".to_string(),
            iterations: 10000,
            warmup_iterations: 1000,
            max_duration: Duration::from_secs(120),
            performance_config: PerformanceConfig {
                enable_object_pooling: true,
                max_pool_size: 5000,
                enable_batch_processing: true,
                batch_size: 100,
                batch_timeout_ms: 10,
                enable_smart_caching: true,
                cache_ttl_seconds: 60,
                max_cache_size: 10000,
                enable_concurrent_processing: true,
                max_concurrent_operations: 50,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Create a low-latency benchmark configuration
    pub fn low_latency() -> Self {
        Self {
            name: "LowLatency".to_string(),
            description: "Low-latency event processing benchmark".to_string(),
            iterations: 5000,
            warmup_iterations: 500,
            max_duration: Duration::from_secs(30),
            performance_config: PerformanceConfig {
                enable_object_pooling: true,
                max_pool_size: 1000,
                enable_batch_processing: false, // Disable batching for lowest latency
                enable_smart_caching: true,
                cache_ttl_seconds: 30,
                max_cache_size: 1000,
                enable_concurrent_processing: true,
                max_concurrent_operations: 10,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Create a memory-efficient benchmark configuration
    pub fn memory_efficient() -> Self {
        Self {
            name: "MemoryEfficient".to_string(),
            description: "Memory-efficient processing benchmark".to_string(),
            iterations: 2000,
            warmup_iterations: 200,
            max_duration: Duration::from_secs(45),
            performance_config: PerformanceConfig {
                enable_object_pooling: true,
                max_pool_size: 100,
                enable_batch_processing: true,
                batch_size: 20,
                batch_timeout_ms: 50,
                enable_smart_caching: false, // Disable caching to save memory
                enable_concurrent_processing: false, // Single-threaded for memory efficiency
                max_concurrent_operations: 1,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

/// Detailed benchmark results with statistical analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub config: BenchmarkConfig,
    pub start_time: String,
    pub total_duration: Duration,
    pub iterations_completed: usize,
    pub throughput_ops_per_sec: f64,
    pub latency_stats: LatencyStatistics,
    pub memory_stats: MemoryStatistics,
    pub performance_metrics: Option<crate::agent::memory::performance::PerformanceSummary>,
    pub error_count: u32,
    pub success_rate: f64,
}

/// Statistical analysis of latency measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStatistics {
    pub min_ms: f64,
    pub max_ms: f64,
    pub mean_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub std_dev_ms: f64,
}

/// Memory usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatistics {
    pub initial_mb: f64,
    pub peak_mb: f64,
    pub final_mb: f64,
    pub average_mb: f64,
    pub growth_mb: f64,
    pub gc_collections: u32,
}

/// Object pooling performance benchmark
pub struct ObjectPoolBenchmark {
    config: BenchmarkConfig,
}

impl ObjectPoolBenchmark {
    pub fn new(config: BenchmarkConfig) -> Self {
        Self { config }
    }

    pub async fn run_benchmark(&self) -> Result<BenchmarkResult, String> {
        info!("Starting Object Pool Benchmark: {}", self.config.name);
        
        let start_time = Instant::now();
        let memory_monitor = MemoryMonitor::new();
        let performance_metrics = Arc::new(PerformanceMetrics::default());
        
        // Create object pool
        let pool = Arc::new(ObjectPool::new(
            || Vec::<u8>::with_capacity(1024), // 1KB objects
            self.config.performance_config.max_pool_size,
            performance_metrics.clone(),
        ));

        // Pre-warm the pool
        pool.pre_warm(self.config.performance_config.max_pool_size / 2).await;
        
        let mut latency_measurements = Vec::with_capacity(self.config.iterations);
        let mut error_count = 0;

        // Warmup phase
        info!("Running {} warmup iterations", self.config.warmup_iterations);
        for _ in 0..self.config.warmup_iterations {
            let start = Instant::now();
            let _obj = pool.get().await;
            let _duration = start.elapsed();
            // Don't record warmup measurements
        }

        memory_monitor.record_measurement("warmup_complete").await;

        // Benchmark phase
        info!("Running {} benchmark iterations", self.config.iterations);
        for i in 0..self.config.iterations {
            let iteration_start = Instant::now();
            
            match self.run_single_iteration(&pool).await {
                Ok(duration) => {
                    latency_measurements.push(duration);
                }
                Err(_) => {
                    error_count += 1;
                }
            }

            // Check timeout
            if start_time.elapsed() > self.config.max_duration {
                warn!("Benchmark timed out after {} iterations", i + 1);
                break;
            }

            // Record memory periodically
            if i % 100 == 0 {
                memory_monitor.record_measurement(&format!("iteration_{}", i)).await;
            }
        }

        let total_duration = start_time.elapsed();
        let iterations_completed = latency_measurements.len();
        
        // Calculate statistics
        let latency_stats = self.calculate_latency_statistics(&latency_measurements);
        let memory_stats = self.calculate_memory_statistics(&memory_monitor).await;
        let throughput = iterations_completed as f64 / total_duration.as_secs_f64();
        let success_rate = (iterations_completed as f64) / (iterations_completed + error_count as usize) as f64;

        info!("Object Pool Benchmark completed: {} ops/sec", throughput);

        Ok(BenchmarkResult {
            config: self.config.clone(),
            start_time: chrono::Utc::now().to_rfc3339(),
            total_duration,
            iterations_completed,
            throughput_ops_per_sec: throughput,
            latency_stats,
            memory_stats,
            performance_metrics: Some(performance_metrics.get_summary()),
            error_count,
            success_rate,
        })
    }

    async fn run_single_iteration(&self, pool: &Arc<ObjectPool<Vec<u8>>>) -> Result<Duration, String> {
        let start = Instant::now();
        
        // Get object from pool
        let mut obj = pool.get().await;
        
        // Simulate some work
        obj.clear();
        obj.extend_from_slice(&[1u8; 100]);
        
        // Simulate processing time
        tokio::time::sleep(Duration::from_micros(10)).await;
        
        Ok(start.elapsed())
    }

    fn calculate_latency_statistics(&self, measurements: &[Duration]) -> LatencyStatistics {
        if measurements.is_empty() {
            return LatencyStatistics {
                min_ms: 0.0,
                max_ms: 0.0,
                mean_ms: 0.0,
                median_ms: 0.0,
                p95_ms: 0.0,
                p99_ms: 0.0,
                std_dev_ms: 0.0,
            };
        }

        let mut sorted: Vec<f64> = measurements.iter()
            .map(|d| d.as_secs_f64() * 1000.0)
            .collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let len = sorted.len();
        let sum: f64 = sorted.iter().sum();
        let mean = sum / len as f64;
        
        let variance = sorted.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / len as f64;
        let std_dev = variance.sqrt();

        LatencyStatistics {
            min_ms: sorted[0],
            max_ms: sorted[len - 1],
            mean_ms: mean,
            median_ms: sorted[len / 2],
            p95_ms: sorted[(len as f64 * 0.95) as usize],
            p99_ms: sorted[(len as f64 * 0.99) as usize],
            std_dev_ms: std_dev,
        }
    }

    async fn calculate_memory_statistics(&self, monitor: &MemoryMonitor) -> MemoryStatistics {
        let stats = monitor.get_statistics().await;
        
        MemoryStatistics {
            initial_mb: stats.initial_memory as f64 / (1024.0 * 1024.0),
            peak_mb: stats.peak_memory as f64 / (1024.0 * 1024.0),
            final_mb: stats.current_memory as f64 / (1024.0 * 1024.0),
            average_mb: stats.avg_memory as f64 / (1024.0 * 1024.0),
            growth_mb: stats.memory_growth as f64 / (1024.0 * 1024.0),
            gc_collections: 0, // Would need platform-specific implementation
        }
    }
}

/// Smart cache performance benchmark
pub struct SmartCacheBenchmark {
    config: BenchmarkConfig,
}

impl SmartCacheBenchmark {
    pub fn new(config: BenchmarkConfig) -> Self {
        Self { config }
    }

    pub async fn run_benchmark(&self) -> Result<BenchmarkResult, String> {
        info!("Starting Smart Cache Benchmark: {}", self.config.name);
        
        let start_time = Instant::now();
        let memory_monitor = MemoryMonitor::new();
        let performance_metrics = Arc::new(PerformanceMetrics::default());
        
        // Create smart cache
        let cache = Arc::new(SmartCache::new(
            Duration::from_secs(self.config.performance_config.cache_ttl_seconds),
            self.config.performance_config.max_cache_size,
            performance_metrics.clone(),
        ));

        let mut latency_measurements = Vec::with_capacity(self.config.iterations);
        let mut error_count = 0;

        // Pre-populate cache for realistic testing
        for i in 0..self.config.performance_config.max_cache_size / 2 {
            cache.put(format!("key_{}", i), format!("value_{}", i)).await;
        }

        // Warmup phase
        for _ in 0..self.config.warmup_iterations {
            let start = Instant::now();
            let _ = self.run_cache_operation(&cache).await;
            let _duration = start.elapsed();
        }

        memory_monitor.record_measurement("warmup_complete").await;

        // Benchmark phase
        for i in 0..self.config.iterations {
            let start = Instant::now();
            
            match self.run_cache_operation(&cache).await {
                Ok(_) => {
                    latency_measurements.push(start.elapsed());
                }
                Err(_) => {
                    error_count += 1;
                }
            }

            if start_time.elapsed() > self.config.max_duration {
                break;
            }

            if i % 100 == 0 {
                memory_monitor.record_measurement(&format!("iteration_{}", i)).await;
            }
        }

        let total_duration = start_time.elapsed();
        let iterations_completed = latency_measurements.len();
        
        let latency_stats = self.calculate_latency_statistics(&latency_measurements);
        let memory_stats = self.calculate_memory_statistics(&memory_monitor).await;
        let throughput = iterations_completed as f64 / total_duration.as_secs_f64();
        let success_rate = (iterations_completed as f64) / (iterations_completed + error_count as usize) as f64;

        info!("Smart Cache Benchmark completed: {} ops/sec", throughput);

        Ok(BenchmarkResult {
            config: self.config.clone(),
            start_time: chrono::Utc::now().to_rfc3339(),
            total_duration,
            iterations_completed,
            throughput_ops_per_sec: throughput,
            latency_stats,
            memory_stats,
            performance_metrics: Some(performance_metrics.get_summary()),
            error_count,
            success_rate,
        })
    }

    async fn run_cache_operation(&self, cache: &Arc<SmartCache<String, String>>) -> Result<(), String> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::from_entropy();
        
        // Mix of cache operations: 70% get, 20% put, 10% cleanup
        let operation = rng.gen_range(0..100);
        
        match operation {
            0..=69 => {
                // Cache get operation
                let key = format!("key_{}", rng.gen_range(0..self.config.performance_config.max_cache_size));
                let _ = cache.get(&key).await;
            }
            70..=89 => {
                // Cache put operation
                let key = format!("key_{}", rng.gen_range(0..self.config.performance_config.max_cache_size * 2));
                let value = format!("value_{}", rng.gen_range(0..1000));
                cache.put(key, value).await;
            }
            90..=99 => {
                // Cache cleanup operation
                let _ = cache.cleanup_expired().await;
            }
            _ => unreachable!(),
        }
        
        Ok(())
    }

    fn calculate_latency_statistics(&self, measurements: &[Duration]) -> LatencyStatistics {
        // Same implementation as ObjectPoolBenchmark
        if measurements.is_empty() {
            return LatencyStatistics {
                min_ms: 0.0,
                max_ms: 0.0,
                mean_ms: 0.0,
                median_ms: 0.0,
                p95_ms: 0.0,
                p99_ms: 0.0,
                std_dev_ms: 0.0,
            };
        }

        let mut sorted: Vec<f64> = measurements.iter()
            .map(|d| d.as_secs_f64() * 1000.0)
            .collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let len = sorted.len();
        let sum: f64 = sorted.iter().sum();
        let mean = sum / len as f64;
        
        let variance = sorted.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / len as f64;
        let std_dev = variance.sqrt();

        LatencyStatistics {
            min_ms: sorted[0],
            max_ms: sorted[len - 1],
            mean_ms: mean,
            median_ms: sorted[len / 2],
            p95_ms: sorted[(len as f64 * 0.95) as usize],
            p99_ms: sorted[(len as f64 * 0.99) as usize],
            std_dev_ms: std_dev,
        }
    }

    async fn calculate_memory_statistics(&self, monitor: &MemoryMonitor) -> MemoryStatistics {
        let stats = monitor.get_statistics().await;
        
        MemoryStatistics {
            initial_mb: stats.initial_memory as f64 / (1024.0 * 1024.0),
            peak_mb: stats.peak_memory as f64 / (1024.0 * 1024.0),
            final_mb: stats.current_memory as f64 / (1024.0 * 1024.0),
            average_mb: stats.avg_memory as f64 / (1024.0 * 1024.0),
            growth_mb: stats.memory_growth as f64 / (1024.0 * 1024.0),
            gc_collections: 0,
        }
    }
}

/// Comprehensive benchmark suite runner
pub struct BenchmarkSuite {
    pub name: String,
    pub benchmarks: Vec<Box<dyn BenchmarkRunner>>,
}

#[async_trait::async_trait]
pub trait BenchmarkRunner: Send + Sync {
    async fn run(&self) -> Result<BenchmarkResult, String>;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
}

#[async_trait::async_trait]
impl BenchmarkRunner for ObjectPoolBenchmark {
    async fn run(&self) -> Result<BenchmarkResult, String> {
        self.run_benchmark().await
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    fn description(&self) -> &str {
        &self.config.description
    }
}

#[async_trait::async_trait]
impl BenchmarkRunner for SmartCacheBenchmark {
    async fn run(&self) -> Result<BenchmarkResult, String> {
        self.run_benchmark().await
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    fn description(&self) -> &str {
        &self.config.description
    }
}

impl BenchmarkSuite {
    pub fn new(name: String) -> Self {
        Self {
            name,
            benchmarks: Vec::new(),
        }
    }

    pub fn add_benchmark(&mut self, benchmark: Box<dyn BenchmarkRunner>) {
        self.benchmarks.push(benchmark);
    }

    pub async fn run_all(&self) -> Vec<BenchmarkResult> {
        let mut results = Vec::new();
        
        info!("Starting Benchmark Suite: {}", self.name);
        info!("Running {} benchmarks", self.benchmarks.len());

        for benchmark in &self.benchmarks {
            info!("Running benchmark: {}", benchmark.name());
            
            match benchmark.run().await {
                Ok(result) => {
                    info!("✅ {} completed: {:.2} ops/sec", 
                         benchmark.name(), result.throughput_ops_per_sec);
                    results.push(result);
                }
                Err(e) => {
                    warn!("❌ {} failed: {}", benchmark.name(), e);
                }
            }
        }

        info!("Benchmark Suite completed: {}/{} successful", 
              results.len(), self.benchmarks.len());
        
        results
    }

    pub fn generate_report(&self, results: &[BenchmarkResult]) -> BenchmarkReport {
        let total_operations: usize = results.iter().map(|r| r.iterations_completed).sum();
        let avg_throughput = results.iter().map(|r| r.throughput_ops_per_sec).sum::<f64>() / results.len() as f64;
        let avg_latency = results.iter().map(|r| r.latency_stats.mean_ms).sum::<f64>() / results.len() as f64;

        BenchmarkReport {
            suite_name: self.name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            total_benchmarks: self.benchmarks.len(),
            successful_benchmarks: results.len(),
            total_operations,
            average_throughput_ops_sec: avg_throughput,
            average_latency_ms: avg_latency,
            individual_results: results.to_vec(),
        }
    }
}

/// Summary report for the entire benchmark suite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub suite_name: String,
    pub timestamp: String,
    pub total_benchmarks: usize,
    pub successful_benchmarks: usize,
    pub total_operations: usize,
    pub average_throughput_ops_sec: f64,
    pub average_latency_ms: f64,
    pub individual_results: Vec<BenchmarkResult>,
}

impl BenchmarkReport {
    pub fn print_summary(&self) {
        println!("\n=== Benchmark Suite Report: {} ===", self.suite_name);
        println!("Timestamp: {}", self.timestamp);
        println!("Benchmarks: {}/{} successful", self.successful_benchmarks, self.total_benchmarks);
        println!("Total Operations: {}", self.total_operations);
        println!("Average Throughput: {:.2} ops/sec", self.average_throughput_ops_sec);
        println!("Average Latency: {:.2} ms", self.average_latency_ms);
        
        println!("\n--- Individual Results ---");
        for result in &self.individual_results {
            println!("Benchmark: {}", result.config.name);
            println!("  Throughput: {:.2} ops/sec", result.throughput_ops_per_sec);
            println!("  Latency: {:.2}ms (mean), {:.2}ms (p95), {:.2}ms (p99)", 
                     result.latency_stats.mean_ms, result.latency_stats.p95_ms, result.latency_stats.p99_ms);
            println!("  Memory: {:.2}MB (peak), {:.2}MB (growth)", 
                     result.memory_stats.peak_mb, result.memory_stats.growth_mb);
            println!("  Success Rate: {:.1}%", result.success_rate * 100.0);
            println!();
        }
    }

    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        let json_data = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize report: {}", e))?;
        
        std::fs::write(path, json_data)
            .map_err(|e| format!("Failed to write report to {}: {}", path, e))?;
        
        info!("Benchmark report saved to: {}", path);
        Ok(())
    }
}

/// Create a comprehensive benchmark suite for performance testing
pub fn create_performance_benchmark_suite() -> BenchmarkSuite {
    let mut suite = BenchmarkSuite::new("TARS Performance Optimization Benchmarks".to_string());

    // Object pool benchmarks
    suite.add_benchmark(Box::new(ObjectPoolBenchmark::new(
        BenchmarkConfig::high_throughput()
    )));
    
    suite.add_benchmark(Box::new(ObjectPoolBenchmark::new(
        BenchmarkConfig::low_latency()
    )));
    
    suite.add_benchmark(Box::new(ObjectPoolBenchmark::new(
        BenchmarkConfig::memory_efficient()
    )));

    // Smart cache benchmarks
    suite.add_benchmark(Box::new(SmartCacheBenchmark::new(
        BenchmarkConfig::high_throughput()
    )));
    
    suite.add_benchmark(Box::new(SmartCacheBenchmark::new(
        BenchmarkConfig::low_latency()
    )));

    suite
}