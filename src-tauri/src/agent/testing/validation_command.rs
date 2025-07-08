//! Validation Command Interface
//!
//! TARS Phase 3.6.5: Final validation and integration testing
//!
//! Command-line interface for running the comprehensive validation suite.

use std::path::Path;
use clap::{Arg, Command, ArgMatches};
use tracing::{info, error};

use super::validation_suite::{ValidationSuite, ValidationConfig, PerformanceThresholds};

/// Validation command configuration
#[derive(Debug, Clone)]
pub struct ValidationCommandConfig {
    pub output_file: Option<String>,
    pub report_file: Option<String>,
    pub verbose: bool,
    pub quick_mode: bool,
    pub validation_config: ValidationConfig,
}

/// Validation command handler
pub struct ValidationCommand;

impl ValidationCommand {
    /// Create the CLI command definition
    pub fn create_command() -> Command {
        Command::new("validate")
            .about("Run comprehensive TARS validation suite")
            .arg(
                Arg::new("output")
                    .short('o')
                    .long("output")
                    .value_name("FILE")
                    .help("Output file for validation results (JSON)")
            )
            .arg(
                Arg::new("report")
                    .short('r')
                    .long("report")
                    .value_name("FILE")
                    .help("Output file for human-readable report")
            )
            .arg(
                Arg::new("quick")
                    .short('q')
                    .long("quick")
                    .help("Run quick validation (reduced test coverage)")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                Arg::new("functional-only")
                    .long("functional-only")
                    .help("Run only functional validation tests")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                Arg::new("performance-only")
                    .long("performance-only")
                    .help("Run only performance validation tests")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                Arg::new("reliability-only")
                    .long("reliability-only")
                    .help("Run only reliability validation tests")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                Arg::new("min-throughput")
                    .long("min-throughput")
                    .value_name("OPS_PER_SEC")
                    .help("Minimum required throughput (ops/sec)")
                    .value_parser(clap::value_parser!(f64))
            )
            .arg(
                Arg::new("max-latency")
                    .long("max-latency")
                    .value_name("MILLISECONDS")
                    .help("Maximum allowed latency (ms)")
                    .value_parser(clap::value_parser!(f64))
            )
            .arg(
                Arg::new("max-memory")
                    .long("max-memory")
                    .value_name("MEGABYTES")
                    .help("Maximum allowed memory usage (MB)")
                    .value_parser(clap::value_parser!(f64))
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
    pub fn parse_config(matches: &ArgMatches) -> Result<ValidationCommandConfig, String> {
        let mut validation_config = ValidationConfig::default();

        // Handle specific validation types
        if matches.get_flag("functional-only") {
            validation_config.enable_all = false;
            validation_config.enable_functional = true;
            validation_config.enable_performance = false;
            validation_config.enable_reliability = false;
            validation_config.enable_security = false;
            validation_config.enable_scalability = false;
        } else if matches.get_flag("performance-only") {
            validation_config.enable_all = false;
            validation_config.enable_functional = false;
            validation_config.enable_performance = true;
            validation_config.enable_reliability = false;
            validation_config.enable_security = false;
            validation_config.enable_scalability = false;
        } else if matches.get_flag("reliability-only") {
            validation_config.enable_all = false;
            validation_config.enable_functional = false;
            validation_config.enable_performance = false;
            validation_config.enable_reliability = true;
            validation_config.enable_security = false;
            validation_config.enable_scalability = false;
        }

        // Handle quick mode
        if matches.get_flag("quick") {
            validation_config.max_test_duration = std::time::Duration::from_secs(120); // 2 minutes
            validation_config.enable_security = false; // Skip security in quick mode
            validation_config.enable_scalability = false; // Skip scalability in quick mode
        }

        // Handle performance thresholds
        if let Some(&min_throughput) = matches.get_one::<f64>("min-throughput") {
            validation_config.performance_thresholds.min_throughput_ops_sec = min_throughput;
        }

        if let Some(&max_latency) = matches.get_one::<f64>("max-latency") {
            validation_config.performance_thresholds.max_avg_latency_ms = max_latency;
        }

        if let Some(&max_memory) = matches.get_one::<f64>("max-memory") {
            validation_config.performance_thresholds.max_memory_usage_mb = max_memory;
        }

        Ok(ValidationCommandConfig {
            output_file: matches.get_one::<String>("output").cloned(),
            report_file: matches.get_one::<String>("report").cloned(),
            verbose: matches.get_flag("verbose"),
            quick_mode: matches.get_flag("quick"),
            validation_config,
        })
    }

    /// Execute the validation command
    pub async fn execute(config: ValidationCommandConfig) -> Result<(), String> {
        info!("Starting TARS validation suite");

        if config.verbose {
            println!("Validation Configuration:");
            println!("  Functional: {}", config.validation_config.enable_functional);
            println!("  Performance: {}", config.validation_config.enable_performance);
            println!("  Reliability: {}", config.validation_config.enable_reliability);
            println!("  Security: {}", config.validation_config.enable_security);
            println!("  Scalability: {}", config.validation_config.enable_scalability);
            println!("  Quick Mode: {}", config.quick_mode);
            println!();
        }

        // Create and run validation suite
        let validation_suite = ValidationSuite::new(config.validation_config);
        
        println!("🚀 Running TARS validation suite...");
        let start_time = std::time::Instant::now();
        
        let results = match validation_suite.run_validation().await {
            Ok(results) => results,
            Err(e) => {
                error!("Validation suite failed: {}", e);
                return Err(format!("Validation suite failed: {}", e));
            }
        };

        let duration = start_time.elapsed();

        // Print results
        match results.overall_status {
            super::validation_suite::ValidationStatus::Pass => {
                println!("✅ Validation PASSED in {:?}", duration);
            }
            super::validation_suite::ValidationStatus::Warning => {
                println!("⚠️  Validation completed with WARNINGS in {:?}", duration);
            }
            super::validation_suite::ValidationStatus::Fail => {
                println!("❌ Validation FAILED in {:?}", duration);
            }
            super::validation_suite::ValidationStatus::Error => {
                println!("💥 Validation ERROR in {:?}", duration);
            }
        }

        println!("📊 Results Summary:");
        println!("  Total Tests: {}", results.summary.total_tests);
        println!("  Passed: {}", results.summary.tests_passed);
        println!("  Failed: {}", results.summary.tests_failed);
        println!("  Success Rate: {:.1}%", results.summary.success_rate);
        println!("  Certification Level: {:?}", results.summary.certification_level);

        if !results.summary.critical_issues.is_empty() {
            println!("\n⚠️  Critical Issues:");
            for issue in &results.summary.critical_issues {
                println!("    • {}", issue);
            }
        }

        if !results.summary.recommendations.is_empty() {
            println!("\n💡 Recommendations:");
            for recommendation in &results.summary.recommendations {
                println!("    • {}", recommendation);
            }
        }

        // Save results if requested
        if let Some(output_file) = &config.output_file {
            validation_suite.save_results(&results, output_file)?;
            println!("\n📄 Results saved to: {}", output_file);
        }

        // Generate and save report if requested
        if let Some(report_file) = &config.report_file {
            let report = validation_suite.generate_report(&results);
            std::fs::write(report_file, report)
                .map_err(|e| format!("Failed to write report to {}: {}", report_file, e))?;
            println!("📋 Report saved to: {}", report_file);
        } else if config.verbose {
            // Print report to console in verbose mode
            println!("\n{}", validation_suite.generate_report(&results));
        }

        // Exit with appropriate code
        match results.overall_status {
            super::validation_suite::ValidationStatus::Pass => Ok(()),
            super::validation_suite::ValidationStatus::Warning => {
                println!("\n⚠️  Validation completed with warnings. Review issues before production deployment.");
                Ok(())
            }
            super::validation_suite::ValidationStatus::Fail | 
            super::validation_suite::ValidationStatus::Error => {
                println!("\n❌ Validation failed. System is not ready for production deployment.");
                Err("Validation failed".to_string())
            }
        }
    }

    /// Quick validation for CI/CD pipelines
    pub async fn quick_validate() -> Result<bool, String> {
        let config = ValidationCommandConfig {
            output_file: None,
            report_file: None,
            verbose: false,
            quick_mode: true,
            validation_config: ValidationConfig {
                enable_all: false,
                enable_functional: true,
                enable_performance: true,
                enable_reliability: false,
                enable_security: false,
                enable_scalability: false,
                performance_thresholds: PerformanceThresholds::default(),
                max_test_duration: std::time::Duration::from_secs(60),
                enable_detailed_reporting: false,
            },
        };

        let validation_suite = ValidationSuite::new(config.validation_config);
        let results = validation_suite.run_validation().await?;

        let passed = matches!(results.overall_status, 
            super::validation_suite::ValidationStatus::Pass | 
            super::validation_suite::ValidationStatus::Warning
        );

        if !passed {
            eprintln!("Quick validation failed:");
            for issue in &results.summary.critical_issues {
                eprintln!("  - {}", issue);
            }
        }

        Ok(passed)
    }

    /// Pre-deployment validation check
    pub async fn pre_deployment_check() -> Result<super::validation_suite::CertificationLevel, String> {
        let config = ValidationConfig {
            enable_all: true,
            performance_thresholds: PerformanceThresholds {
                min_throughput_ops_sec: 200.0,  // Higher requirements for production
                max_avg_latency_ms: 50.0,       // Stricter latency requirements
                max_memory_usage_mb: 256.0,     // Stricter memory requirements
                min_cache_hit_rate: 90.0,       // Higher cache hit rate required
                max_error_rate: 0.1,            // Very low error rate required
            },
            max_test_duration: std::time::Duration::from_secs(300), // 5 minutes
            enable_detailed_reporting: true,
        };

        let validation_suite = ValidationSuite::new(config);
        let results = validation_suite.run_validation().await?;

        println!("Pre-deployment validation completed:");
        println!("  Status: {:?}", results.overall_status);
        println!("  Certification Level: {:?}", results.summary.certification_level);
        println!("  Success Rate: {:.1}%", results.summary.success_rate);

        if !results.summary.critical_issues.is_empty() {
            println!("  Critical Issues: {}", results.summary.critical_issues.len());
        }

        Ok(results.summary.certification_level)
    }
}