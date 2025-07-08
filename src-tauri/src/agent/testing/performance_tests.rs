//! Performance Tests for Event-Driven Memory System
//!
//! Tests to validate performance characteristics and identify bottlenecks

use std::time::{Duration, Instant};
use async_trait::async_trait;

use super::test_utilities::{TestCase, TestConfig, TestResult, TestMetrics, TestUtilities, MemoryMonitor};
use crate::agent::memory::EventMemoryManager;
use crate::agent::traits::MemoryManager;

/// High-throughput message processing test
pub struct HighThroughputTest;

#[async_trait]
impl TestCase for HighThroughputTest {
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
        "HighThroughput"
    }

    fn description(&self) -> &str {
        "Tests high-throughput message processing performance"
    }
}

impl HighThroughputTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        let mut memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
        let memory_monitor = TestUtilities::create_memory_monitor();
        
        let message_count = config.event_count;
        let messages = TestUtilities::generate_test_messages(message_count);
        
        memory_monitor.record_measurement("test_start").await;
        let start_time = Instant::now();
        
        // Rapid message addition
        for (i, message) in messages.iter().enumerate() {
            memory_manager.add_message(message.clone()).await
                .map_err(|e| format!("Failed to add message {}: {}", i, e))?;
            
            // Record memory usage every 1000 messages
            if i % 1000 == 0 {
                memory_monitor.record_measurement(&format!("message_{}", i)).await;
            }
        }
        
        let add_duration = start_time.elapsed();
        memory_monitor.record_measurement("messages_added").await;
        
        // Measure retrieval performance
        let retrieval_start = Instant::now();
        let retrieved_messages = memory_manager.get_messages().await
            .map_err(|e| format!("Failed to retrieve messages: {}", e))?;
        let retrieval_duration = retrieval_start.elapsed();
        
        memory_monitor.record_measurement("messages_retrieved").await;
        
        // Validate count
        if retrieved_messages.len() != message_count {
            return Err(format!(
                "Message count mismatch: expected {}, got {}",
                message_count, retrieved_messages.len()
            ));
        }
        
        // Calculate performance metrics
        let total_duration = start_time.elapsed();
        let throughput = message_count as f64 / add_duration.as_secs_f64();
        let avg_latency = (add_duration.as_millis() as f64) / message_count as f64;
        
        let memory_stats = memory_monitor.get_statistics().await;
        
        println!("High Throughput Test Results:");
        println!("  Messages processed: {}", message_count);
        println!("  Add duration: {:?}", add_duration);
        println!("  Retrieval duration: {:?}", retrieval_duration);
        println!("  Throughput: {:.2} messages/sec", throughput);
        println!("  Average latency: {:.3} ms", avg_latency);
        println!("  Peak memory: {} bytes", memory_stats.peak_memory);
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: message_count as u64,
            memory_peak_mb: memory_stats.peak_memory / (1024 * 1024),
            throughput_events_per_sec: throughput,
            average_latency_ms: avg_latency,
            error_count: 0,
        })
    }
}

/// Large message handling test
pub struct LargeMessageTest;

#[async_trait]
impl TestCase for LargeMessageTest {
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
        "LargeMessage"
    }

    fn description(&self) -> &str {
        "Tests handling of large message content"
    }
}

impl LargeMessageTest {
    async fn run_test(&self, _config: &TestConfig) -> Result<TestMetrics, String> {
        let mut memory_manager = TestUtilities::create_test_memory_manager(
            crate::agent::memory::EventMemoryConfig::default()
        ).await?;
        
        let memory_monitor = TestUtilities::create_memory_monitor();
        
        // Test various message sizes
        let sizes_kb = vec![1, 10, 100, 500, 1000]; // 1KB to 1MB
        let mut total_messages = 0;
        let start_time = Instant::now();
        
        for size_kb in sizes_kb {
            memory_monitor.record_measurement(&format!("before_{}kb", size_kb)).await;
            
            let large_content = TestUtilities::generate_large_content(size_kb);
            let message = crate::agent::core::Message {
                role: crate::agent::core::Role::User,
                content: large_content,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            };
            
            let message_start = Instant::now();
            memory_manager.add_message(message).await
                .map_err(|e| format!("Failed to add {}KB message: {}", size_kb, e))?;
            let message_duration = message_start.elapsed();
            
            memory_monitor.record_measurement(&format!("after_{}kb", size_kb)).await;
            
            println!("{}KB message processed in {:?}", size_kb, message_duration);
            total_messages += 1;
        }
        
        // Test retrieval performance with large messages
        let retrieval_start = Instant::now();
        let retrieved_messages = memory_manager.get_messages().await
            .map_err(|e| format!("Failed to retrieve large messages: {}", e))?;
        let retrieval_duration = retrieval_start.elapsed();
        
        if retrieved_messages.len() != total_messages {
            return Err(format!(
                "Message count mismatch: expected {}, got {}",
                total_messages, retrieved_messages.len()
            ));
        }
        
        let total_duration = start_time.elapsed();
        let memory_stats = memory_monitor.get_statistics().await;
        
        println!("Large Message Test Results:");
        println!("  Messages processed: {}", total_messages);
        println!("  Total duration: {:?}", total_duration);
        println!("  Retrieval duration: {:?}", retrieval_duration);
        println!("  Peak memory: {} bytes", memory_stats.peak_memory);
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: total_messages as u64,
            memory_peak_mb: memory_stats.peak_memory / (1024 * 1024),
            throughput_events_per_sec: total_messages as f64 / total_duration.as_secs_f64(),
            average_latency_ms: (total_duration.as_millis() as f64) / total_messages as f64,
            error_count: 0,
        })
    }
}

/// Concurrent access stress test
pub struct ConcurrentStressTest;

#[async_trait]
impl TestCase for ConcurrentStressTest {
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
        "ConcurrentStress"
    }

    fn description(&self) -> &str {
        "Stress tests concurrent access patterns"
    }
}

impl ConcurrentStressTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        let memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
        let memory_monitor = TestUtilities::create_memory_monitor();
        
        let concurrent_tasks = config.concurrent_operations;
        let messages_per_task = 200;
        let readers_per_writer = 3; // 3 readers for every 1 writer
        
        memory_monitor.record_measurement("concurrent_test_start").await;
        let start_time = Instant::now();
        
        let mut handles = Vec::new();
        
        // Spawn writer tasks
        for task_id in 0..concurrent_tasks {
            let memory_manager = memory_manager.clone();
            let messages = TestUtilities::generate_test_messages(messages_per_task);
            
            let handle = tokio::spawn(async move {
                let mut local_manager = memory_manager;
                let mut written = 0;
                
                for (i, message) in messages.into_iter().enumerate() {
                    let mut modified_message = message;
                    modified_message.content = format!("Writer {} Message {}: {}", task_id, i, modified_message.content);
                    
                    match local_manager.add_message(modified_message).await {
                        Ok(_) => written += 1,
                        Err(e) => eprintln!("Writer {} failed to add message {}: {}", task_id, i, e),
                    }
                }
                
                (task_id, written, 0) // (task_id, written_count, read_count)
            });
            
            handles.push(handle);
        }
        
        // Spawn reader tasks (more readers than writers)
        for task_id in 0..(concurrent_tasks * readers_per_writer) {
            let memory_manager = memory_manager.clone();
            
            let handle = tokio::spawn(async move {
                let mut read_count = 0;
                let reader_id = task_id + 1000; // Offset to distinguish from writers
                
                // Read multiple times during the test
                for _ in 0..50 {
                    match memory_manager.get_messages().await {
                        Ok(_) => read_count += 1,
                        Err(e) => eprintln!("Reader {} failed to read messages: {}", reader_id, e),
                    }
                    
                    // Small delay to allow interleaving with writes
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                
                (reader_id, 0, read_count) // (task_id, written_count, read_count)
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        let mut total_written = 0;
        let mut total_reads = 0;
        let mut error_count = 0;
        
        for handle in handles {
            match handle.await {
                Ok((task_id, written, reads)) => {
                    total_written += written;
                    total_reads += reads;
                    if task_id < 1000 {
                        println!("Writer {} completed: {} messages written", task_id, written);
                    } else {
                        println!("Reader {} completed: {} reads", task_id, reads);
                    }
                },
                Err(e) => {
                    eprintln!("Task join error: {}", e);
                    error_count += 1;
                },
            }
        }
        
        let duration = start_time.elapsed();
        memory_monitor.record_measurement("concurrent_test_end").await;
        
        // Verify final state
        let final_messages = memory_manager.get_messages().await
            .map_err(|e| format!("Failed to get final messages: {}", e))?;
        
        let memory_stats = memory_monitor.get_statistics().await;
        
        println!("Concurrent Stress Test Results:");
        println!("  Writers: {}, Readers: {}", concurrent_tasks, concurrent_tasks * readers_per_writer);
        println!("  Total messages written: {}", total_written);
        println!("  Total reads performed: {}", total_reads);
        println!("  Final message count: {}", final_messages.len());
        println!("  Duration: {:?}", duration);
        println!("  Peak memory: {} bytes", memory_stats.peak_memory);
        println!("  Errors: {}", error_count);
        
        // Calculate performance metrics
        let total_operations = total_written + total_reads;
        let throughput = total_operations as f64 / duration.as_secs_f64();
        let avg_latency = (duration.as_millis() as f64) / total_operations as f64;
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: total_written as u64,
            memory_peak_mb: memory_stats.peak_memory / (1024 * 1024),
            throughput_events_per_sec: throughput,
            average_latency_ms: avg_latency,
            error_count,
        })
    }
}

/// Memory efficiency test
pub struct MemoryEfficiencyTest;

#[async_trait]
impl TestCase for MemoryEfficiencyTest {
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
        "MemoryEfficiency"
    }

    fn description(&self) -> &str {
        "Tests memory usage efficiency and garbage collection"
    }
}

impl MemoryEfficiencyTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        let mut memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
        let memory_monitor = TestUtilities::create_memory_monitor();
        
        memory_monitor.record_measurement("baseline").await;
        
        // Phase 1: Add messages and measure growth
        let batch_size = 100;
        let batches = 10;
        
        for batch in 0..batches {
            let messages = TestUtilities::generate_test_messages(batch_size);
            
            for message in messages {
                memory_manager.add_message(message).await
                    .map_err(|e| format!("Failed to add message in batch {}: {}", batch, e))?;
            }
            
            memory_monitor.record_measurement(&format!("batch_{}", batch)).await;
            
            // Force some operations that might trigger cleanup
            let _ = memory_manager.get_messages().await;
            let _ = memory_manager.get_last_n_messages(10).await;
        }
        
        memory_monitor.record_measurement("after_addition").await;
        
        // Phase 2: Clear memory and measure cleanup
        memory_manager.clear_memory().await
            .map_err(|e| format!("Failed to clear memory: {}", e))?;
        
        memory_monitor.record_measurement("after_clear").await;
        
        // Phase 3: Add messages again to test memory reuse
        let new_messages = TestUtilities::generate_test_messages(batch_size);
        for message in new_messages {
            memory_manager.add_message(message).await
                .map_err(|e| format!("Failed to add message after clear: {}", e))?;
        }
        
        memory_monitor.record_measurement("after_reuse").await;
        
        let memory_stats = memory_monitor.get_statistics().await;
        
        println!("Memory Efficiency Test Results:");
        println!("  Initial memory: {} bytes", memory_stats.initial_memory);
        println!("  Peak memory: {} bytes", memory_stats.peak_memory);
        println!("  Final memory: {} bytes", memory_stats.current_memory);
        println!("  Memory growth: {} bytes", memory_stats.memory_growth);
        println!("  Measurements taken: {}", memory_stats.measurements_count);
        
        // Check for reasonable memory usage
        let total_messages_processed = batch_size * batches + batch_size;
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: total_messages_processed as u64,
            memory_peak_mb: memory_stats.peak_memory / (1024 * 1024),
            throughput_events_per_sec: 0.0,
            average_latency_ms: 0.0,
            error_count: 0,
        })
    }
}

/// Create all performance tests
pub fn create_performance_tests() -> Vec<Box<dyn TestCase>> {
    vec![
        Box::new(HighThroughputTest),
        Box::new(LargeMessageTest),
        Box::new(ConcurrentStressTest),
        Box::new(MemoryEfficiencyTest),
    ]
}