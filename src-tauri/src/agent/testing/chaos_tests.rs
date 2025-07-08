//! Chaos Tests for Resilience Validation
//!
//! Tests that introduce various failure scenarios to validate system resilience

use std::time::{Duration, Instant};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use async_trait::async_trait;
use rand::{Rng, SeedableRng};

use super::test_utilities::{TestCase, TestConfig, TestResult, TestMetrics, TestUtilities};
use crate::agent::memory::EventMemoryManager;
use crate::agent::traits::MemoryManager;

/// Chaos test that introduces random failures
pub struct RandomFailureTest;

#[async_trait]
impl TestCase for RandomFailureTest {
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
        "RandomFailure"
    }

    fn description(&self) -> &str {
        "Tests system resilience with random operation failures"
    }
}

impl RandomFailureTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        let mut memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
        
        let total_operations = 1000;
        let failure_rate = 0.1; // 10% failure rate
        let mut rng = rand::rngs::StdRng::seed_from_u64(42); // Use seeded RNG for Send compatibility
        
        let mut successful_operations = 0;
        let mut failed_operations = 0;
        let start_time = Instant::now();
        
        for i in 0..total_operations {
            // Randomly decide if this operation should "fail"
            let should_simulate_failure = rng.gen::<f64>() < failure_rate;
            
            if should_simulate_failure {
                // Simulate failure by trying to add invalid data or perform invalid operations
                failed_operations += 1;
                
                // Try some operations that might fail gracefully
                let _ = memory_manager.get_last_n_messages(0).await; // Invalid count
                
                continue;
            }
            
            // Normal operation
            let message = crate::agent::core::Message {
                role: crate::agent::core::Role::User,
                content: format!("Message {} in chaos test", i),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            };
            
            match memory_manager.add_message(message).await {
                Ok(_) => successful_operations += 1,
                Err(_) => failed_operations += 1,
            }
            
            // Occasionally clear memory to test recovery
            if i % 100 == 0 && i > 0 {
                let _ = memory_manager.clear_memory().await;
            }
        }
        
        let duration = start_time.elapsed();
        
        // Verify system is still functional
        let final_messages = memory_manager.get_messages().await
            .map_err(|e| format!("System not functional after chaos test: {}", e))?;
        
        println!("Random Failure Test Results:");
        println!("  Total operations: {}", total_operations);
        println!("  Successful operations: {}", successful_operations);
        println!("  Failed operations: {}", failed_operations);
        println!("  Final message count: {}", final_messages.len());
        println!("  System remained functional: ✅");
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: successful_operations as u64,
            memory_peak_mb: 0,
            throughput_events_per_sec: successful_operations as f64 / duration.as_secs_f64(),
            average_latency_ms: (duration.as_millis() as f64) / total_operations as f64,
            error_count: failed_operations as u32,
        })
    }
}

/// Interruption test that simulates sudden stops and restarts
pub struct InterruptionTest;

#[async_trait]
impl TestCase for InterruptionTest {
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
        "Interruption"
    }

    fn description(&self) -> &str {
        "Tests recovery from sudden interruptions and restarts"
    }
}

impl InterruptionTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        if !config.enable_persistence {
            return Err("Persistence required for interruption test".to_string());
        }
        
        let interruption_cycles = 5;
        let messages_per_cycle = 100;
        let mut total_messages_processed = 0;
        
        for cycle in 0..interruption_cycles {
            // Create memory manager
            let mut memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
            
            // Start a session
            let session_id = memory_manager.start_new_session().await
                .map_err(|e| format!("Failed to start session in cycle {}: {}", cycle, e))?;
            
            // Add some messages
            let messages = TestUtilities::generate_test_messages(messages_per_cycle);
            for (i, message) in messages.iter().enumerate() {
                memory_manager.add_message(message.clone()).await
                    .map_err(|e| format!("Failed to add message {} in cycle {}: {}", i, cycle, e))?;
                
                total_messages_processed += 1;
                
                // Simulate interruption in the middle
                if i == messages_per_cycle / 2 {
                    // Force checkpoint before "interruption"
                    memory_manager.checkpoint_current_session().await
                        .map_err(|e| format!("Failed to checkpoint in cycle {}: {}", cycle, e))?;
                    
                    // Simulate interruption by dropping the memory manager
                    drop(memory_manager);
                    
                    // Create new memory manager (simulating restart)
                    memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
                    
                    // Recover the session
                    let recovered_messages = memory_manager.load_session(&session_id).await
                        .map_err(|e| format!("Failed to recover session in cycle {}: {}", cycle, e))?;
                    
                    if recovered_messages.len() != i {
                        return Err(format!(
                            "Recovery failed in cycle {}: expected {} messages, got {}",
                            cycle, i, recovered_messages.len()
                        ));
                    }
                }
            }
            
            // Final checkpoint
            memory_manager.checkpoint_current_session().await
                .map_err(|e| format!("Failed final checkpoint in cycle {}: {}", cycle, e))?;
            
            // Verify session integrity
            let final_session_messages = memory_manager.load_session(&session_id).await
                .map_err(|e| format!("Failed to load final session in cycle {}: {}", cycle, e))?;
            
            if final_session_messages.len() != messages_per_cycle {
                return Err(format!(
                    "Session integrity check failed in cycle {}: expected {} messages, got {}",
                    cycle, messages_per_cycle, final_session_messages.len()
                ));
            }
            
            // Clean up
            memory_manager.delete_session(&session_id).await
                .map_err(|e| format!("Failed to delete session in cycle {}: {}", cycle, e))?;
        }
        
        println!("Interruption Test Results:");
        println!("  Interruption cycles: {}", interruption_cycles);
        println!("  Messages per cycle: {}", messages_per_cycle);
        println!("  Total messages processed: {}", total_messages_processed);
        println!("  All recoveries successful: ✅");
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: total_messages_processed as u64,
            memory_peak_mb: 0,
            throughput_events_per_sec: 0.0,
            average_latency_ms: 0.0,
            error_count: 0,
        })
    }
}

/// Race condition test
pub struct RaceConditionTest;

#[async_trait]
impl TestCase for RaceConditionTest {
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
        "RaceCondition"
    }

    fn description(&self) -> &str {
        "Tests for race conditions in concurrent access patterns"
    }
}

impl RaceConditionTest {
    async fn run_test(&self, config: &TestConfig) -> Result<TestMetrics, String> {
        let memory_manager = TestUtilities::create_test_memory_manager(config.memory_config.clone()).await?;
        let should_stop = Arc::new(AtomicBool::new(false));
        
        let concurrent_tasks = 20;
        let test_duration = Duration::from_secs(10);
        let operations_per_task = 100;
        
        // Start timer
        let stop_flag = should_stop.clone();
        tokio::spawn(async move {
            tokio::time::sleep(test_duration).await;
            stop_flag.store(true, Ordering::Relaxed);
        });
        
        let mut handles = Vec::new();
        
        // Spawn tasks that perform different types of operations concurrently
        for task_id in 0..concurrent_tasks {
            let memory_manager = memory_manager.clone();
            let should_stop = should_stop.clone();
            
            let handle = tokio::spawn(async move {
                let mut memory_manager = memory_manager.clone();
                let mut operations_count = 0;
                let mut errors = 0;
                
                while !should_stop.load(Ordering::Relaxed) && operations_count < operations_per_task {
                    let operation_type = operations_count % 4;
                    
                    match operation_type {
                        0 => {
                            // Add message
                            let message = crate::agent::core::Message {
                                role: crate::agent::core::Role::User,
                                content: format!("Task {} message {}", task_id, operations_count),
                                tool_calls: None,
                                tool_call_id: None,
                                name: None,
                            };
                            
                            if let Err(_) = memory_manager.add_message(message).await {
                                errors += 1;
                            }
                        },
                        1 => {
                            // Get messages
                            if let Err(_) = memory_manager.get_messages().await {
                                errors += 1;
                            }
                        },
                        2 => {
                            // Get last N messages
                            if let Err(_) = memory_manager.get_last_n_messages(10).await {
                                errors += 1;
                            }
                        },
                        3 => {
                            // Clear memory (occasionally)
                            if operations_count % 50 == 0 {
                                if let Err(_) = memory_manager.clear_memory().await {
                                    errors += 1;
                                }
                            }
                        },
                        _ => unreachable!(),
                    }
                    
                    operations_count += 1;
                    
                    // Small random delay to increase chance of race conditions
                    let mut local_rng = rand::rngs::StdRng::seed_from_u64(task_id as u64 + operations_count as u64);
                    let delay_ms = local_rng.gen_range(0..5);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
                
                (task_id, operations_count, errors)
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks
        let mut total_operations = 0;
        let mut total_errors = 0;
        
        for handle in handles {
            match handle.await {
                Ok((task_id, ops, errors)) => {
                    total_operations += ops;
                    total_errors += errors;
                    println!("Task {} completed: {} operations, {} errors", task_id, ops, errors);
                },
                Err(e) => {
                    println!("Task failed: {}", e);
                    total_errors += 1;
                },
            }
        }
        
        // Verify system integrity
        let final_messages = memory_manager.get_messages().await
            .map_err(|e| format!("System integrity check failed: {}", e))?;
        
        println!("Race Condition Test Results:");
        println!("  Concurrent tasks: {}", concurrent_tasks);
        println!("  Total operations: {}", total_operations);
        println!("  Total errors: {}", total_errors);
        println!("  Final message count: {}", final_messages.len());
        println!("  System integrity maintained: ✅");
        
        // Check error rate
        let error_rate = total_errors as f64 / total_operations as f64;
        if error_rate > 0.1 { // More than 10% error rate indicates serious issues
            return Err(format!("High error rate detected: {:.2}%", error_rate * 100.0));
        }
        
        Ok(TestMetrics {
            events_processed: 0,
            messages_processed: total_operations as u64,
            memory_peak_mb: 0,
            throughput_events_per_sec: total_operations as f64 / test_duration.as_secs_f64(),
            average_latency_ms: 0.0,
            error_count: total_errors as u32,
        })
    }
}

/// Create chaos tests
pub fn create_chaos_tests() -> Vec<Box<dyn TestCase>> {
    vec![
        Box::new(RandomFailureTest),
        Box::new(InterruptionTest),
        Box::new(RaceConditionTest),
    ]
}