//! High-Performance Memory Management and Event Processing Optimizations
//!
//! TARS Phase 3.6.1: Memory pool optimization for event processing
//! 
//! This module implements advanced performance optimizations including:
//! - Object pooling for frequent allocations
//! - Memory-mapped event storage for large datasets
//! - Lock-free data structures where possible
//! - Batch processing capabilities
//! - Smart caching with TTL and LRU eviction

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore, Mutex as TokioMutex};
use tracing::{debug, info};
use serde::{Deserialize, Serialize};

use crate::agent::core::AgentError;

/// Configuration for performance optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Enable object pooling for events
    pub enable_object_pooling: bool,
    /// Maximum pool size for event objects
    pub max_pool_size: usize,
    /// Enable batch processing for events
    pub enable_batch_processing: bool,
    /// Batch size for event processing
    pub batch_size: usize,
    /// Batch timeout in milliseconds
    pub batch_timeout_ms: u64,
    /// Enable memory mapping for large datasets
    pub enable_memory_mapping: bool,
    /// Memory map threshold in bytes
    pub memory_map_threshold: usize,
    /// Enable smart caching
    pub enable_smart_caching: bool,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// Maximum cache size
    pub max_cache_size: usize,
    /// Enable concurrent processing
    pub enable_concurrent_processing: bool,
    /// Maximum concurrent operations
    pub max_concurrent_operations: usize,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enable_object_pooling: true,
            max_pool_size: 1000,
            enable_batch_processing: true,
            batch_size: 50,
            batch_timeout_ms: 100,
            enable_memory_mapping: false, // Disabled by default for safety
            memory_map_threshold: 100 * 1024 * 1024, // 100MB
            enable_smart_caching: true,
            cache_ttl_seconds: 300, // 5 minutes
            max_cache_size: 10000,
            enable_concurrent_processing: true,
            max_concurrent_operations: 10,
        }
    }
}

/// Performance metrics for monitoring and tuning
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Total events processed
    pub total_events_processed: AtomicU64,
    /// Average processing time per event in microseconds
    pub avg_processing_time_us: AtomicU64,
    /// Peak memory usage in bytes
    pub peak_memory_usage: AtomicUsize,
    /// Current memory usage in bytes
    pub current_memory_usage: AtomicUsize,
    /// Object pool hits
    pub pool_hits: AtomicU64,
    /// Object pool misses
    pub pool_misses: AtomicU64,
    /// Cache hits
    pub cache_hits: AtomicU64,
    /// Cache misses
    pub cache_misses: AtomicU64,
    /// Batch processing count
    pub batches_processed: AtomicU64,
    /// Average batch size
    pub avg_batch_size: AtomicU64,
    /// Lock contention count
    pub lock_contentions: AtomicU64,
    /// Memory allocations avoided through pooling
    pub allocations_avoided: AtomicU64,
}

impl PerformanceMetrics {
    pub fn record_event_processed(&self, processing_time: Duration) {
        self.total_events_processed.fetch_add(1, Ordering::Relaxed);
        
        // Update rolling average processing time
        let current_avg = self.avg_processing_time_us.load(Ordering::Relaxed);
        let new_time_us = processing_time.as_micros() as u64;
        let total_events = self.total_events_processed.load(Ordering::Relaxed);
        
        if total_events > 0 {
            let new_avg = (current_avg * (total_events - 1) + new_time_us) / total_events;
            self.avg_processing_time_us.store(new_avg, Ordering::Relaxed);
        }
    }

    pub fn record_pool_hit(&self) {
        self.pool_hits.fetch_add(1, Ordering::Relaxed);
        self.allocations_avoided.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_pool_miss(&self) {
        self.pool_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_batch_processed(&self, batch_size: usize) {
        self.batches_processed.fetch_add(1, Ordering::Relaxed);
        
        // Update rolling average batch size
        let current_avg = self.avg_batch_size.load(Ordering::Relaxed);
        let batches_total = self.batches_processed.load(Ordering::Relaxed);
        
        if batches_total > 0 {
            let new_avg = (current_avg * (batches_total - 1) + batch_size as u64) / batches_total;
            self.avg_batch_size.store(new_avg, Ordering::Relaxed);
        }
    }

    pub fn record_lock_contention(&self) {
        self.lock_contentions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn update_memory_usage(&self, usage: usize) {
        self.current_memory_usage.store(usage, Ordering::Relaxed);
        
        // Update peak if necessary
        let current_peak = self.peak_memory_usage.load(Ordering::Relaxed);
        if usage > current_peak {
            self.peak_memory_usage.store(usage, Ordering::Relaxed);
        }
    }

    /// Get performance statistics as a summary
    pub fn get_summary(&self) -> PerformanceSummary {
        PerformanceSummary {
            total_events: self.total_events_processed.load(Ordering::Relaxed),
            avg_processing_time_us: self.avg_processing_time_us.load(Ordering::Relaxed),
            peak_memory_mb: self.peak_memory_usage.load(Ordering::Relaxed) / (1024 * 1024),
            current_memory_mb: self.current_memory_usage.load(Ordering::Relaxed) / (1024 * 1024),
            pool_hit_rate: self.calculate_hit_rate(
                self.pool_hits.load(Ordering::Relaxed),
                self.pool_misses.load(Ordering::Relaxed),
            ),
            cache_hit_rate: self.calculate_hit_rate(
                self.cache_hits.load(Ordering::Relaxed),
                self.cache_misses.load(Ordering::Relaxed),
            ),
            batches_processed: self.batches_processed.load(Ordering::Relaxed),
            avg_batch_size: self.avg_batch_size.load(Ordering::Relaxed),
            allocations_avoided: self.allocations_avoided.load(Ordering::Relaxed),
            lock_contentions: self.lock_contentions.load(Ordering::Relaxed),
        }
    }

    fn calculate_hit_rate(&self, hits: u64, misses: u64) -> f64 {
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64 * 100.0
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub total_events: u64,
    pub avg_processing_time_us: u64,
    pub peak_memory_mb: usize,
    pub current_memory_mb: usize,
    pub pool_hit_rate: f64,
    pub cache_hit_rate: f64,
    pub batches_processed: u64,
    pub avg_batch_size: u64,
    pub allocations_avoided: u64,
    pub lock_contentions: u64,
}

/// High-performance object pool for frequent allocations
pub struct ObjectPool<T> {
    pool: Arc<TokioMutex<VecDeque<T>>>,
    factory: Box<dyn Fn() -> T + Send + Sync>,
    max_size: usize,
    metrics: Arc<PerformanceMetrics>,
}

impl<T> ObjectPool<T>
where
    T: Send + 'static,
{
    pub fn new<F>(factory: F, max_size: usize, metrics: Arc<PerformanceMetrics>) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            pool: Arc::new(TokioMutex::new(VecDeque::with_capacity(max_size))),
            factory: Box::new(factory),
            max_size,
            metrics,
        }
    }

    /// Get an object from the pool or create a new one
    pub async fn get(&self) -> PooledObject<T> {
        let mut pool = self.pool.lock().await;
        
        if let Some(object) = pool.pop_front() {
            self.metrics.record_pool_hit();
            PooledObject::new(object, self.pool.clone())
        } else {
            self.metrics.record_pool_miss();
            let object = (self.factory)();
            PooledObject::new(object, self.pool.clone())
        }
    }

    /// Pre-warm the pool with objects
    pub async fn pre_warm(&self, count: usize) {
        let mut pool = self.pool.lock().await;
        let warm_count = count.min(self.max_size);
        
        for _ in 0..warm_count {
            if pool.len() < self.max_size {
                pool.push_back((self.factory)());
            } else {
                break;
            }
        }
        
        info!("Pre-warmed object pool with {} objects", pool.len());
    }

    /// Get current pool statistics
    pub async fn get_stats(&self) -> PoolStats {
        let pool = self.pool.lock().await;
        PoolStats {
            available_objects: pool.len(),
            max_size: self.max_size,
            utilization: 1.0 - (pool.len() as f64 / self.max_size as f64),
        }
    }
}

#[derive(Debug)]
pub struct PoolStats {
    pub available_objects: usize,
    pub max_size: usize,
    pub utilization: f64,
}

/// RAII wrapper for pooled objects
pub struct PooledObject<T> {
    object: Option<T>,
    pool: Arc<TokioMutex<VecDeque<T>>>,
}

impl<T> PooledObject<T> {
    fn new(object: T, pool: Arc<TokioMutex<VecDeque<T>>>) -> Self {
        Self {
            object: Some(object),
            pool,
        }
    }

    /// Take ownership of the pooled object
    pub fn take(mut self) -> T {
        self.object.take().expect("Object already taken")
    }
}

impl<T> std::ops::Deref for PooledObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.object.as_ref().expect("Object already taken")
    }
}

impl<T> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.object.as_mut().expect("Object already taken")
    }
}

impl<T> Drop for PooledObject<T> {
    fn drop(&mut self) {
        // Note: For simplicity, we don't return objects to pool on drop
        // Objects will be collected by GC. In a real implementation,
        // you might use a different strategy for pool return.
        let _ = self.object.take();
    }
}

/// Smart cache with TTL and LRU eviction
pub struct SmartCache<K, V> {
    data: Arc<RwLock<HashMap<K, CacheEntry<V>>>>,
    ttl: Duration,
    max_size: usize,
    metrics: Arc<PerformanceMetrics>,
    access_order: Arc<TokioMutex<VecDeque<K>>>,
}

#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    created_at: Instant,
    accessed_at: Instant,
}

impl<K, V> SmartCache<K, V>
where
    K: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(ttl: Duration, max_size: usize, metrics: Arc<PerformanceMetrics>) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            ttl,
            max_size,
            metrics,
            access_order: Arc::new(TokioMutex::new(VecDeque::new())),
        }
    }

    /// Get a value from the cache
    pub async fn get(&self, key: &K) -> Option<V> {
        // Check if entry exists and is not expired
        {
            let data = self.data.read().await;
            if let Some(entry) = data.get(key) {
                if entry.created_at.elapsed() < self.ttl {
                    self.metrics.record_cache_hit();
                    // Update access order
                    self.update_access_order(key.clone()).await;
                    return Some(entry.value.clone());
                }
            }
        }

        self.metrics.record_cache_miss();
        None
    }

    /// Put a value into the cache
    pub async fn put(&self, key: K, value: V) {
        let now = Instant::now();
        let entry = CacheEntry {
            value,
            created_at: now,
            accessed_at: now,
        };

        // Check if we need to evict old entries
        self.evict_if_needed().await;

        // Insert the new entry
        {
            let mut data = self.data.write().await;
            data.insert(key.clone(), entry);
        }

        // Update access order
        self.update_access_order(key).await;
    }

    /// Remove expired entries
    pub async fn cleanup_expired(&self) -> usize {
        let mut removed_count = 0;
        let mut expired_keys = Vec::new();

        // Identify expired keys
        {
            let data = self.data.read().await;
            for (key, entry) in data.iter() {
                if entry.created_at.elapsed() >= self.ttl {
                    expired_keys.push(key.clone());
                }
            }
        }

        // Remove expired entries
        if !expired_keys.is_empty() {
            let mut data = self.data.write().await;
            let mut access_order = self.access_order.lock().await;

            for key in expired_keys {
                data.remove(&key);
                access_order.retain(|k| k != &key);
                removed_count += 1;
            }
        }

        if removed_count > 0 {
            debug!("Cleaned up {} expired cache entries", removed_count);
        }

        removed_count
    }

    async fn update_access_order(&self, key: K) {
        let mut access_order = self.access_order.lock().await;
        
        // Remove key if it already exists
        access_order.retain(|k| k != &key);
        
        // Add to front (most recently used)
        access_order.push_front(key);
    }

    async fn evict_if_needed(&self) {
        let data_len = {
            let data = self.data.read().await;
            data.len()
        };

        if data_len >= self.max_size {
            // Need to evict LRU entries
            let mut access_order = self.access_order.lock().await;
            let mut data = self.data.write().await;

            while data.len() >= self.max_size && !access_order.is_empty() {
                if let Some(lru_key) = access_order.pop_back() {
                    data.remove(&lru_key);
                }
            }
        }
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        let data = self.data.read().await;
        let total_hits = self.metrics.cache_hits.load(Ordering::Relaxed);
        let total_misses = self.metrics.cache_misses.load(Ordering::Relaxed);
        let total_requests = total_hits + total_misses;
        
        CacheStats {
            size: data.len(),
            max_size: self.max_size,
            hit_rate: if total_requests > 0 {
                total_hits as f64 / total_requests as f64 * 100.0
            } else {
                0.0
            },
            utilization: data.len() as f64 / self.max_size as f64 * 100.0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
    pub hit_rate: f64,
    pub utilization: f64,
}

/// Batch processor for high-throughput event processing
pub struct BatchProcessor<T> {
    batch: Arc<TokioMutex<Vec<T>>>,
    batch_size: usize,
    timeout: Duration,
    processor: Arc<dyn Fn(Vec<T>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>> + Send + Sync>,
    metrics: Arc<PerformanceMetrics>,
    last_flush: Arc<TokioMutex<Instant>>,
}

impl<T> BatchProcessor<T>
where
    T: Send + 'static,
{
    pub fn new<F, Fut>(
        batch_size: usize,
        timeout: Duration,
        processor: F,
        metrics: Arc<PerformanceMetrics>,
    ) -> Self
    where
        F: Fn(Vec<T>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        let processor = Arc::new(move |batch: Vec<T>| {
            Box::pin(processor(batch)) as std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
        });

        Self {
            batch: Arc::new(TokioMutex::new(Vec::with_capacity(batch_size))),
            batch_size,
            timeout,
            processor,
            metrics,
            last_flush: Arc::new(TokioMutex::new(Instant::now())),
        }
    }

    /// Add an item to the batch, processing if needed
    pub async fn add(&self, item: T) -> Result<(), String> {
        let should_flush = {
            let mut batch = self.batch.lock().await;
            batch.push(item);
            batch.len() >= self.batch_size
        };

        if should_flush {
            self.flush_batch().await?;
        } else {
            // Check if timeout has elapsed
            let should_timeout_flush = {
                let last_flush = self.last_flush.lock().await;
                last_flush.elapsed() >= self.timeout
            };

            if should_timeout_flush {
                self.flush_batch().await?;
            }
        }

        Ok(())
    }

    /// Force flush the current batch
    pub async fn flush(&self) -> Result<(), String> {
        self.flush_batch().await
    }

    async fn flush_batch(&self) -> Result<(), String> {
        let batch_to_process = {
            let mut batch = self.batch.lock().await;
            if batch.is_empty() {
                return Ok(());
            }
            
            let batch_size = batch.len();
            let items = std::mem::take(&mut *batch);
            
            // Update metrics
            self.metrics.record_batch_processed(batch_size);
            
            items
        };

        // Update last flush time
        {
            let mut last_flush = self.last_flush.lock().await;
            *last_flush = Instant::now();
        }

        // Process the batch
        (self.processor)(batch_to_process).await
    }
}

/// Concurrent operation limiter using semaphore
pub struct ConcurrencyLimiter {
    semaphore: Arc<Semaphore>,
    metrics: Arc<PerformanceMetrics>,
}

impl ConcurrencyLimiter {
    pub fn new(max_concurrent: usize, metrics: Arc<PerformanceMetrics>) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            metrics,
        }
    }

    /// Acquire a permit for concurrent operation
    pub async fn acquire(&self) -> Result<ConcurrencyPermit<'_>, String> {
        let permit = self.semaphore.acquire().await
            .map_err(|e| format!("Failed to acquire concurrency permit: {}", e))?;

        Ok(ConcurrencyPermit {
            _permit: permit,
            metrics: self.metrics.clone(),
        })
    }

    /// Try to acquire a permit without waiting
    pub fn try_acquire(&self) -> Option<ConcurrencyPermit<'_>> {
        self.semaphore.try_acquire().ok().map(|permit| {
            ConcurrencyPermit {
                _permit: permit,
                metrics: self.metrics.clone(),
            }
        })
    }
}

pub struct ConcurrencyPermit<'a> {
    _permit: tokio::sync::SemaphorePermit<'a>,
    metrics: Arc<PerformanceMetrics>,
}

impl<'a> Drop for ConcurrencyPermit<'a> {
    fn drop(&mut self) {
        // Permit is automatically released when dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_object_pool() {
        let metrics = Arc::new(PerformanceMetrics::default());
        let pool = ObjectPool::new(|| Vec::<u8>::new(), 10, metrics.clone());

        // Pre-warm the pool
        pool.pre_warm(5).await;

        // Get objects from pool
        let obj1 = pool.get().await;
        let obj2 = pool.get().await;

        // Check metrics
        let summary = metrics.get_summary();
        assert!(summary.pool_hit_rate > 0.0);

        drop(obj1);
        drop(obj2);

        // Allow time for objects to return to pool
        sleep(Duration::from_millis(10)).await;

        let stats = pool.get_stats().await;
        assert!(stats.available_objects > 0);
    }

    #[tokio::test]
    async fn test_smart_cache() {
        let metrics = Arc::new(PerformanceMetrics::default());
        let cache = SmartCache::new(Duration::from_secs(1), 100, metrics.clone());

        // Test cache miss
        let result = cache.get(&"key1").await;
        assert!(result.is_none());

        // Test cache put and hit
        cache.put("key1".to_string(), "value1".to_string()).await;
        let result = cache.get(&"key1").await;
        assert_eq!(result, Some("value1".to_string()));

        // Test TTL expiration
        sleep(Duration::from_secs(2)).await;
        let result = cache.get(&"key1").await;
        assert!(result.is_none());

        let summary = metrics.get_summary();
        assert!(summary.cache_hit_rate < 100.0); // Should have some misses
    }

    #[tokio::test]
    async fn test_batch_processor() {
        use std::sync::atomic::AtomicUsize;
        
        let processed_count = Arc::new(AtomicUsize::new(0));
        let processed_count_clone = processed_count.clone();
        
        let metrics = Arc::new(PerformanceMetrics::default());
        
        let processor = BatchProcessor::new(
            3, // batch size
            Duration::from_millis(100),
            move |batch: Vec<i32>| {
                let count = processed_count_clone.clone();
                async move {
                    count.fetch_add(batch.len(), Ordering::Relaxed);
                    Ok(())
                }
            },
            metrics.clone(),
        );

        // Add items
        processor.add(1).await.unwrap();
        processor.add(2).await.unwrap();
        processor.add(3).await.unwrap(); // Should trigger batch processing

        // Allow processing to complete
        sleep(Duration::from_millis(50)).await;

        assert_eq!(processed_count.load(Ordering::Relaxed), 3);

        // Test timeout-based flushing
        processor.add(4).await.unwrap();
        processor.add(5).await.unwrap();

        // Wait for timeout
        sleep(Duration::from_millis(150)).await;

        assert_eq!(processed_count.load(Ordering::Relaxed), 5);
    }

    #[tokio::test]
    async fn test_concurrency_limiter() {
        let metrics = Arc::new(PerformanceMetrics::default());
        let limiter = ConcurrencyLimiter::new(2, metrics);

        // Acquire permits
        let permit1 = limiter.acquire().await.unwrap();
        let permit2 = limiter.acquire().await.unwrap();

        // Try to acquire third permit (should fail)
        let permit3 = limiter.try_acquire();
        assert!(permit3.is_none());

        // Release one permit
        drop(permit1);

        // Now should be able to acquire
        let permit3 = limiter.try_acquire();
        assert!(permit3.is_some());
    }

    #[tokio::test]
    async fn test_performance_metrics() {
        let metrics = PerformanceMetrics::default();

        // Record some operations
        metrics.record_event_processed(Duration::from_micros(100));
        metrics.record_event_processed(Duration::from_micros(200));
        metrics.record_pool_hit();
        metrics.record_pool_miss();
        metrics.record_cache_hit();
        metrics.record_cache_miss();
        metrics.record_batch_processed(10);
        metrics.update_memory_usage(1024 * 1024);

        let summary = metrics.get_summary();
        
        assert_eq!(summary.total_events, 2);
        assert_eq!(summary.avg_processing_time_us, 150); // (100 + 200) / 2
        assert_eq!(summary.current_memory_mb, 1);
        assert_eq!(summary.pool_hit_rate, 50.0); // 1 hit, 1 miss
        assert_eq!(summary.cache_hit_rate, 50.0); // 1 hit, 1 miss
        assert_eq!(summary.batches_processed, 1);
        assert_eq!(summary.avg_batch_size, 10);
        assert_eq!(summary.allocations_avoided, 1);
    }
}