//! Performance Testing Command Interface
//!
//! TARS Phase 3.6.4: Performance benchmarking and metrics
//!
//! Command-line interface for running comprehensive performance tests,
//! benchmarks, and generating detailed performance reports.

use std::sync::Arc;
use std::time::Duration;
use clap::{Arg, Command, ArgMatches};
use serde_json;
use tracing::{info, warn, error};

use super::benchmark_suite::{
    BenchmarkSuite, BenchmarkConfig, ObjectPoolBenchmark, SmartCacheBenchmark,
    create_performance_benchmark_suite,
};
use super::performance_monitor::{PerformanceMonitor, MonitorConfig};
use super::test_utilities::{TestConfig, TestSuite, TestCase};
use super::integration_tests::create_integration_tests;
use super::performance_tests::create_performance_tests;
use super::chaos_tests::create_chaos_tests;
use super::memory_tests::create_memory_tests;
use super::conversation_tests::create_conversation_tests;
use crate::agent::memory::performance::PerformanceMetrics;

/// Performance testing command configuration
#[derive(Debug, Clone)]
pub struct PerformanceTestConfig {
    pub test_type: PerformanceTestType,
    pub output_format: OutputFormat,
    pub output_file: Option<String>,
    pub verbose: bool,
    pub iterations: Option<usize>,
    pub duration: Option<Duration>,
    pub enable_monitoring: bool,
    pub monitor_interval: Duration,
    pub save_raw_data: bool,
}

#[derive(Debug, Clone)]
pub enum PerformanceTestType {
    Benchmark,
    Monitor,
    FullSuite,
    Integration,
    Stress,
    Memory,
    Comparison,
}

#[derive(Debug, Clone)]
pub enum OutputFormat {
    Human,
    Json,
    Csv,
    Html,
}

/// Performance testing command handler
pub struct PerformanceTestCommand {
    performance_metrics: Arc<PerformanceMetrics>,
}

impl PerformanceTestCommand {
    pub fn new() -> Self {
        Self {
            performance_metrics: Arc::new(PerformanceMetrics::default()),
        }
    }

    /// Create the CLI command definition
    pub fn create_command() -> Command {
        Command::new("perf-test")
            .about("Run comprehensive performance tests and benchmarks")
            .arg(
                Arg::new("type")
                    .short('t')
                    .long("type")
                    .value_name("TYPE")
                    .help("Type of performance test to run")
                    .value_parser(["benchmark", "monitor", "suite", "integration", "stress", "memory", "compare"])
                    .default_value("benchmark")
            )
            .arg(
                Arg::new("output")
                    .short('o')
                    .long("output")
                    .value_name("FILE")
                    .help("Output file for results")
            )
            .arg(
                Arg::new("format")
                    .short('f')
                    .long("format")
                    .value_name("FORMAT")
                    .help("Output format")
                    .value_parser(["human", "json", "csv", "html"])
                    .default_value("human")
            )
            .arg(
                Arg::new("iterations")
                    .short('i')
                    .long("iterations")
                    .value_name("COUNT")
                    .help("Number of iterations to run")
                    .value_parser(clap::value_parser!(usize))
            )
            .arg(
                Arg::new("duration")
                    .short('d')
                    .long("duration")
                    .value_name("SECONDS")
                    .help("Test duration in seconds")
                    .value_parser(clap::value_parser!(u64))
            )
            .arg(
                Arg::new("monitor")
                    .long("monitor")
                    .help("Enable real-time monitoring during tests")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                Arg::new("monitor-interval")
                    .long("monitor-interval")
                    .value_name("SECONDS")
                    .help("Monitoring sample interval in seconds")
                    .value_parser(clap::value_parser!(u64))
                    .default_value("1")
            )
            .arg(
                Arg::new("save-raw")
                    .long("save-raw")
                    .help("Save raw performance data")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                Arg::new("verbose")
                    .short('v')
                    .long("verbose")
                    .help("Enable verbose output")
                    .action(clap::ArgAction::SetTrue)
            )
    }

    /// Parse command line arguments into configuration
    pub fn parse_config(matches: &ArgMatches) -> Result<PerformanceTestConfig, String> {
        let test_type = match matches.get_one::<String>("type").unwrap().as_str() {
            "benchmark" => PerformanceTestType::Benchmark,
            "monitor" => PerformanceTestType::Monitor,
            "suite" => PerformanceTestType::FullSuite,
            "integration" => PerformanceTestType::Integration,
            "stress" => PerformanceTestType::Stress,
            "memory" => PerformanceTestType::Memory,
            "compare" => PerformanceTestType::Comparison,
            other => return Err(format!("Unknown test type: {}", other)),
        };

        let output_format = match matches.get_one::<String>("format").unwrap().as_str() {
            "human" => OutputFormat::Human,
            "json" => OutputFormat::Json,
            "csv" => OutputFormat::Csv,
            "html" => OutputFormat::Html,
            other => return Err(format!("Unknown output format: {}", other)),
        };

        let duration = matches.get_one::<u64>("duration")
            .map(|&secs| Duration::from_secs(secs));

        let monitor_interval = Duration::from_secs(
            *matches.get_one::<u64>("monitor-interval").unwrap()
        );

        Ok(PerformanceTestConfig {
            test_type,
            output_format,
            output_file: matches.get_one::<String>("output").cloned(),
            verbose: matches.get_flag("verbose"),
            iterations: matches.get_one::<usize>("iterations").copied(),
            duration,
            enable_monitoring: matches.get_flag("monitor"),
            monitor_interval,
            save_raw_data: matches.get_flag("save-raw"),
        })
    }

    /// Execute the performance test command
    pub async fn execute(&self, config: PerformanceTestConfig) -> Result<(), String> {
        info!("Starting performance test: {:?}", config.test_type);

        match config.test_type {
            PerformanceTestType::Benchmark => {
                self.run_benchmarks(&config).await
            }
            PerformanceTestType::Monitor => {
                self.run_monitoring(&config).await
            }
            PerformanceTestType::FullSuite => {
                self.run_full_suite(&config).await
            }
            PerformanceTestType::Integration => {
                self.run_integration_tests(&config).await
            }
            PerformanceTestType::Stress => {
                self.run_stress_tests(&config).await
            }
            PerformanceTestType::Memory => {
                self.run_memory_tests(&config).await
            }
            PerformanceTestType::Comparison => {
                self.run_comparison_tests(&config).await
            }
        }
    }

    /// Run performance benchmarks
    async fn run_benchmarks(&self, config: &PerformanceTestConfig) -> Result<(), String> {
        info!("Running performance benchmarks");

        let mut suite = create_performance_benchmark_suite();
        
        // Add custom benchmarks based on configuration
        if let Some(iterations) = config.iterations {
            let mut custom_config = BenchmarkConfig::default();
            custom_config.iterations = iterations;
            
            if let Some(duration) = config.duration {
                custom_config.max_duration = duration;
            }

            suite.add_benchmark(Box::new(ObjectPoolBenchmark::new(custom_config.clone())));
            suite.add_benchmark(Box::new(SmartCacheBenchmark::new(custom_config)));
        }

        let results = suite.run_all().await;
        let report = suite.generate_report(&results);

        self.output_benchmark_results(&report, config).await?;
        
        info!("Benchmark suite completed successfully");
        Ok(())
    }

    /// Run real-time performance monitoring
    async fn run_monitoring(&self, config: &PerformanceTestConfig) -> Result<(), String> {
        info!("Starting real-time performance monitoring");

        let monitor_config = MonitorConfig {
            sample_interval: config.monitor_interval,
            window_size: 300, // 5 minutes
            enable_alerting: true,
            regression_threshold: 20.0,
            min_samples_for_detection: 30,
            track_memory_details: true,
            collect_latency_histograms: true,
        };

        let monitor = PerformanceMonitor::new(monitor_config, self.performance_metrics.clone());
        monitor.start().await?;

        let duration = config.duration.unwrap_or(Duration::from_secs(300)); // Default 5 minutes
        info!("Monitoring for {:?}", duration);

        // Monitor for the specified duration
        tokio::time::sleep(duration).await;

        let summary = monitor.get_current_summary().await;
        let alerts = monitor.get_recent_alerts(Some(10)).await;
        
        monitor.stop().await;

        // Output monitoring results
        self.output_monitoring_results(&summary, &alerts, config).await?;

        info!("Performance monitoring completed");
        Ok(())
    }

    /// Run the full test suite including all test types
    async fn run_full_suite(&self, config: &PerformanceTestConfig) -> Result<(), String> {
        info!("Running full performance test suite");

        let test_config = TestConfig::default();
        
        // Create comprehensive test suite
        let mut suite = TestSuite::new("TARS Performance Test Suite".to_string());
        
        // Add all test categories
        for test in create_integration_tests() {
            suite.add_test(test);
        }
        
        for test in create_performance_tests() {
            suite.add_test(test);
        }
        
        for test in create_chaos_tests() {
            suite.add_test(test);
        }
        
        for test in create_memory_tests() {
            suite.add_test(test);
        }
        
        for test in create_conversation_tests() {
            suite.add_test(test);
        }

        // Run the test suite
        let results = suite.run_all(&test_config).await;
        let report = suite.generate_report(&results);

        // Also run benchmarks
        let benchmark_suite = create_performance_benchmark_suite();
        let benchmark_results = benchmark_suite.run_all().await;
        let benchmark_report = benchmark_suite.generate_report(&benchmark_results);

        // Output combined results
        self.output_full_suite_results(&report, &benchmark_report, config).await?;

        info!("Full test suite completed");
        Ok(())
    }

    /// Run integration tests specifically
    async fn run_integration_tests(&self, config: &PerformanceTestConfig) -> Result<(), String> {
        info!("Running integration tests");

        let test_config = TestConfig::default();
        let mut suite = TestSuite::new("Integration Tests".to_string());
        
        for test in create_integration_tests() {
            suite.add_test(test);
        }

        let results = suite.run_all(&test_config).await;
        let report = suite.generate_report(&results);

        self.output_test_results(&report, config).await?;
        
        info!("Integration tests completed");
        Ok(())
    }

    /// Run stress tests
    async fn run_stress_tests(&self, config: &PerformanceTestConfig) -> Result<(), String> {
        info!("Running stress tests");

        let test_config = TestConfig::stress_test();
        let mut suite = TestSuite::new("Stress Tests".to_string());
        
        for test in create_performance_tests() {
            suite.add_test(test);
        }
        
        for test in create_chaos_tests() {
            suite.add_test(test);
        }

        let results = suite.run_all(&test_config).await;
        let report = suite.generate_report(&results);

        self.output_test_results(&report, config).await?;
        
        info!("Stress tests completed");
        Ok(())
    }

    /// Run memory-focused tests
    async fn run_memory_tests(&self, config: &PerformanceTestConfig) -> Result<(), String> {
        info!("Running memory tests");

        let test_config = TestConfig::memory_leak_test();
        let mut suite = TestSuite::new("Memory Tests".to_string());
        
        for test in create_memory_tests() {
            suite.add_test(test);
        }

        let results = suite.run_all(&test_config).await;
        let report = suite.generate_report(&results);

        self.output_test_results(&report, config).await?;
        
        info!("Memory tests completed");
        Ok(())
    }

    /// Run comparison tests between different configurations
    async fn run_comparison_tests(&self, config: &PerformanceTestConfig) -> Result<(), String> {
        info!("Running comparison tests");

        // Test different performance configurations
        let configs = vec![
            BenchmarkConfig::high_throughput(),
            BenchmarkConfig::low_latency(),
            BenchmarkConfig::memory_efficient(),
        ];

        let mut all_results = Vec::new();

        for bench_config in configs {
            info!("Testing configuration: {}", bench_config.name);
            
            let pool_benchmark = ObjectPoolBenchmark::new(bench_config.clone());
            let cache_benchmark = SmartCacheBenchmark::new(bench_config);
            
            match pool_benchmark.run_benchmark().await {
                Ok(result) => all_results.push(result),
                Err(e) => warn!("Pool benchmark failed: {}", e),
            }
            
            match cache_benchmark.run_benchmark().await {
                Ok(result) => all_results.push(result),
                Err(e) => warn!("Cache benchmark failed: {}", e),
            }
        }

        self.output_comparison_results(&all_results, config).await?;
        
        info!("Comparison tests completed");
        Ok(())
    }

    /// Output benchmark results in the specified format
    async fn output_benchmark_results(
        &self,
        report: &super::benchmark_suite::BenchmarkReport,
        config: &PerformanceTestConfig,
    ) -> Result<(), String> {
        match config.output_format {
            OutputFormat::Human => {
                report.print_summary();
            }
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(report)
                    .map_err(|e| format!("Failed to serialize to JSON: {}", e))?;
                
                if let Some(file) = &config.output_file {
                    std::fs::write(file, json)
                        .map_err(|e| format!("Failed to write to {}: {}", file, e))?;
                    info!("Benchmark results saved to: {}", file);
                } else {
                    println!("{}", json);
                }
            }
            OutputFormat::Csv => {
                // Implement CSV output for benchmark results
                let csv_data = self.benchmark_results_to_csv(report)?;
                
                if let Some(file) = &config.output_file {
                    std::fs::write(file, csv_data)
                        .map_err(|e| format!("Failed to write to {}: {}", file, e))?;
                } else {
                    println!("{}", csv_data);
                }
            }
            OutputFormat::Html => {
                // Implement HTML output for benchmark results
                let html_data = self.benchmark_results_to_html(report)?;
                
                if let Some(file) = &config.output_file {
                    std::fs::write(file, html_data)
                        .map_err(|e| format!("Failed to write to {}: {}", file, e))?;
                } else {
                    println!("{}", html_data);
                }
            }
        }

        if config.save_raw_data {
            if let Some(base_file) = &config.output_file {
                let raw_file = format!("{}.raw.json", base_file);
                report.save_to_file(&raw_file)?;
            }
        }

        Ok(())
    }

    /// Output monitoring results
    async fn output_monitoring_results(
        &self,
        summary: &super::performance_monitor::PerformanceMonitorSummary,
        alerts: &[super::performance_monitor::PerformanceAlert],
        config: &PerformanceTestConfig,
    ) -> Result<(), String> {
        match config.output_format {
            OutputFormat::Human => {
                println!("\n=== Performance Monitoring Summary ===");
                println!("Sample Count: {}", summary.sample_count);
                println!("Average Throughput: {:.2} ops/sec", summary.avg_throughput_ops_sec);
                println!("Average Latency: {:.2} ms", summary.avg_latency_ms);
                println!("Active Alerts: {}", summary.active_alerts);
                
                if !alerts.is_empty() {
                    println!("\n--- Recent Alerts ---");
                    for alert in alerts {
                        println!("{:?}: {} ({})", alert.severity, alert.description, alert.metric_name);
                    }
                }
            }
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "summary": summary,
                    "alerts": alerts
                });
                
                let json = serde_json::to_string_pretty(&output)
                    .map_err(|e| format!("Failed to serialize to JSON: {}", e))?;
                
                if let Some(file) = &config.output_file {
                    std::fs::write(file, json)
                        .map_err(|e| format!("Failed to write to {}: {}", file, e))?;
                } else {
                    println!("{}", json);
                }
            }
            _ => {
                return Err("Only human and JSON output formats are supported for monitoring".to_string());
            }
        }

        Ok(())
    }

    /// Output test results
    async fn output_test_results(
        &self,
        report: &super::test_utilities::TestReport,
        config: &PerformanceTestConfig,
    ) -> Result<(), String> {
        match config.output_format {
            OutputFormat::Human => {
                report.print_summary();
            }
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(report)
                    .map_err(|e| format!("Failed to serialize to JSON: {}", e))?;
                
                if let Some(file) = &config.output_file {
                    std::fs::write(file, json)
                        .map_err(|e| format!("Failed to write to {}: {}", file, e))?;
                } else {
                    println!("{}", json);
                }
            }
            _ => {
                return Err("Only human and JSON output formats are supported for test results".to_string());
            }
        }

        Ok(())
    }

    /// Output full suite results
    async fn output_full_suite_results(
        &self,
        test_report: &super::test_utilities::TestReport,
        benchmark_report: &super::benchmark_suite::BenchmarkReport,
        config: &PerformanceTestConfig,
    ) -> Result<(), String> {
        match config.output_format {
            OutputFormat::Human => {
                test_report.print_summary();
                benchmark_report.print_summary();
            }
            OutputFormat::Json => {
                let combined = serde_json::json!({
                    "test_results": test_report,
                    "benchmark_results": benchmark_report
                });
                
                let json = serde_json::to_string_pretty(&combined)
                    .map_err(|e| format!("Failed to serialize to JSON: {}", e))?;
                
                if let Some(file) = &config.output_file {
                    std::fs::write(file, json)
                        .map_err(|e| format!("Failed to write to {}: {}", file, e))?;
                } else {
                    println!("{}", json);
                }
            }
            _ => {
                return Err("Only human and JSON output formats are supported for full suite".to_string());
            }
        }

        Ok(())
    }

    /// Output comparison results
    async fn output_comparison_results(
        &self,
        results: &[super::benchmark_suite::BenchmarkResult],
        config: &PerformanceTestConfig,
    ) -> Result<(), String> {
        match config.output_format {
            OutputFormat::Human => {
                println!("\n=== Performance Comparison Results ===");
                for result in results {
                    println!("Configuration: {}", result.config.name);
                    println!("  Throughput: {:.2} ops/sec", result.throughput_ops_per_sec);
                    println!("  Latency: {:.2}ms (mean), {:.2}ms (p95)", 
                             result.latency_stats.mean_ms, result.latency_stats.p95_ms);
                    println!("  Memory: {:.2}MB (peak)", result.memory_stats.peak_mb);
                    println!();
                }
            }
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(results)
                    .map_err(|e| format!("Failed to serialize to JSON: {}", e))?;
                
                if let Some(file) = &config.output_file {
                    std::fs::write(file, json)
                        .map_err(|e| format!("Failed to write to {}: {}", file, e))?;
                } else {
                    println!("{}", json);
                }
            }
            _ => {
                return Err("Only human and JSON output formats are supported for comparison".to_string());
            }
        }

        Ok(())
    }

    /// Convert benchmark results to CSV format
    fn benchmark_results_to_csv(&self, report: &super::benchmark_suite::BenchmarkReport) -> Result<String, String> {
        let mut csv = String::new();
        csv.push_str("benchmark,throughput_ops_sec,mean_latency_ms,p95_latency_ms,p99_latency_ms,peak_memory_mb,success_rate\n");
        
        for result in &report.individual_results {
            csv.push_str(&format!(
                "{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.3}\n",
                result.config.name,
                result.throughput_ops_per_sec,
                result.latency_stats.mean_ms,
                result.latency_stats.p95_ms,
                result.latency_stats.p99_ms,
                result.memory_stats.peak_mb,
                result.success_rate
            ));
        }
        
        Ok(csv)
    }

    /// Convert benchmark results to HTML format
    fn benchmark_results_to_html(&self, report: &super::benchmark_suite::BenchmarkReport) -> Result<String, String> {
        let mut html = String::new();
        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n<title>Benchmark Results</title>\n");
        html.push_str("<style>table { border-collapse: collapse; width: 100%; } th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }</style>\n");
        html.push_str("</head>\n<body>\n");
        html.push_str(&format!("<h1>Benchmark Suite: {}</h1>\n", report.suite_name));
        html.push_str("<table>\n");
        html.push_str("<tr><th>Benchmark</th><th>Throughput (ops/sec)</th><th>Mean Latency (ms)</th><th>P95 Latency (ms)</th><th>Peak Memory (MB)</th><th>Success Rate</th></tr>\n");
        
        for result in &report.individual_results {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td><td>{:.1}%</td></tr>\n",
                result.config.name,
                result.throughput_ops_per_sec,
                result.latency_stats.mean_ms,
                result.latency_stats.p95_ms,
                result.memory_stats.peak_mb,
                result.success_rate * 100.0
            ));
        }
        
        html.push_str("</table>\n</body>\n</html>");
        Ok(html)
    }
}