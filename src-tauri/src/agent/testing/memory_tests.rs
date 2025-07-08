//! Memory-specific Tests
//!
//! Tests focused on memory management, leak detection, and resource cleanup

use std::time::{Duration, Instant};
use async_trait::async_trait;

use super::test_utilities::{TestCase, TestConfig, TestResult, TestMetrics, TestUtilities, MemoryMonitor};
use crate::agent::memory::EventMemoryManager;
use crate::agent::traits::MemoryManager;

/// Memory leak detection test
pub struct MemoryLeakTest;

#[async_trait]
impl TestCase for MemoryLeakTest {
    async fn run(&self, config: &TestConfig) -> TestResult {
        let start_time = Instant::now();
        
        match self.run_test(config).await {
            Ok(metrics) => TestResult::success(
                self.name().to_string(),
                start_time.elapsed(),
                Some(metrics),
            ),
            Err(error) => TestResult::failure(
                self.name().to_string(),
                start_time.elapsed(),
                error,
            ),
        }
    }

    fn name(&self) -> &str {
        "MemoryLeak"
    }

    fn description(&self) -> &str {
        "Detects memory leaks during repeated operations"
    }
}

impl MemoryLeakTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        let memory_monitor = TestUtilities::create_memory_monitor();
        let cycles = 10;
        let messages_per_cycle = 1000;
        
        memory_monitor.record_measurement("baseline").await;
        
        for cycle in 0..cycles {
            // Create a new memory manager for each cycle
            let mut memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
            
            memory_monitor.record_measurement(&format!("cycle_{}_start", cycle)).await;
            
            // Add messages
            let messages = TestUtilities::generate_test_messages(messages_per_cycle);
            for message in messages {
                memory_manager.add_message(message).await
                    .map_err(|e| format!("Failed to add message in cycle {}: {}", cycle, e))?;
            }
            
            // Perform some operations
            let _ = memory_manager.get_messages().await;
            let _ = memory_manager.get_last_n_messages(10).await;
            
            // Clear and cleanup
            memory_manager.clear_memory().await
                .map_err(|e| format!("Failed to clear memory in cycle {}: {}", cycle, e))?;
            
            memory_monitor.record_measurement(&format!("cycle_{}_end", cycle)).await;
            
            // Drop the memory manager to ensure cleanup
            drop(memory_manager);
            
            // Give time for cleanup
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        memory_monitor.record_measurement("final").await;
        
        let memory_stats = memory_monitor.get_statistics().await;
        
        println!("Memory Leak Test Results:");
        println!("  Cycles: {}", cycles);
        println!("  Messages per cycle: {}", messages_per_cycle);
        println!("  Initial memory: {} bytes", memory_stats.initial_memory);
        println!("  Final memory: {} bytes", memory_stats.current_memory);
        println!("  Peak memory: {} bytes", memory_stats.peak_memory);
        println!("  Memory growth: {} bytes", memory_stats.memory_growth);
        
        // Check for significant memory growth (potential leak)
        let growth_threshold = 10 * 1024 * 1024; // 10MB
        if memory_stats.memory_growth > growth_threshold {
            return Err(format!(
                "Potential memory leak detected: {} bytes growth exceeds threshold of {} bytes",
                memory_stats.memory_growth, growth_threshold
            ));
        }
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: (cycles * messages_per_cycle) as u64,
            memory_peak_mb: memory_stats.peak_memory / (1024 * 1024),
            throughput_events_per_sec: 0.0,
            average_latency_ms: 0.0,
            error_count: 0,
        })
    }
}

/// Resource cleanup test
pub struct ResourceCleanupTest;

#[async_trait]
impl TestCase for ResourceCleanupTest {
    async fn run(&self, config: &TestConfig) -> TestResult {
        let start_time = Instant::now();
        
        match self.run_test(config).await {
            Ok(metrics) => TestResult::success(
                self.name().to_string(),
                start_time.elapsed(),
                Some(metrics),
            ),
            Err(error) => TestResult::failure(
                self.name().to_string(),
                start_time.elapsed(),
                error,
            ),
        }
    }

    fn name(&self) -> &str {
        "ResourceCleanup"
    }

    fn description(&self) -> &str {
        "Tests proper cleanup of resources and handles"
    }
}

impl ResourceCleanupTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        let memory_monitor = TestUtilities::create_memory_monitor();
        memory_monitor.record_measurement("start").await;
        
        // Test multiple memory manager instances
        let mut managers = Vec::new();
        
        for i in 0..10 {
            let memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
            managers.push(memory_manager);
            memory_monitor.record_measurement(&format!("manager_{}_created", i)).await;
        }
        
        // Use all managers
        for (i, manager) in managers.iter_mut().enumerate() {
            let messages = TestUtilities::generate_test_messages(100);
            for message in messages {
                manager.add_message(message).await
                    .map_err(|e| format!("Failed to add message to manager {}: {}", i, e))?;
            }
            memory_monitor.record_measurement(&format!("manager_{}_used", i)).await;
        }
        
        // Clear all managers
        for (i, manager) in managers.iter_mut().enumerate() {
            manager.clear_memory().await
                .map_err(|e| format!("Failed to clear manager {}: {}", i, e))?;
            memory_monitor.record_measurement(&format!("manager_{}_cleared", i)).await;
        }
        
        // Drop all managers
        drop(managers);
        memory_monitor.record_measurement("all_dropped").await;
        
        // Allow time for cleanup
        tokio::time::sleep(Duration::from_secs(1)).await;
        memory_monitor.record_measurement("after_cleanup").await;
        
        let memory_stats = memory_monitor.get_statistics().await;
        
        println!("Resource Cleanup Test Results:");
        println!("  Memory managers created: 10");
        println!("  Initial memory: {} bytes", memory_stats.initial_memory);
        println!("  Final memory: {} bytes", memory_stats.current_memory);
        println!("  Peak memory: {} bytes", memory_stats.peak_memory);
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: 1000, // 10 managers * 100 messages
            memory_peak_mb: memory_stats.peak_memory / (1024 * 1024),
            throughput_events_per_sec: 0.0,
            average_latency_ms: 0.0,
            error_count: 0,
        })
    }
}

/// Create memory-specific tests
pub fn create_memory_tests() -> Vec<Box<dyn TestCase>> {
    vec![
        Box::new(MemoryLeakTest),
        Box::new(ResourceCleanupTest),
    ]
}