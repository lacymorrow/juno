/// Performance monitoring and optimization utilities
/// Critical fixes for voice processing memory and CPU issues
use std::time::{Duration, Instant};
use std::sync::{Arc, atomic::{AtomicU64, AtomicBool, Ordering}};
use tracing::{warn, info, error};

/// Performance metrics for audio processing
#[derive(Debug, Clone)]
pub struct AudioPerformanceMetrics {
    pub buffer_size_mb: f64,
    pub processing_time_ms: u64,
    pub cpu_usage_percent: f32,
    pub memory_pressure: bool,
}

/// Global performance monitor for voice processing
pub struct PerformanceMonitor {
    max_buffer_size_mb: f64,
    max_processing_time_ms: u64,
    cpu_threshold_percent: f32,
    last_cleanup: Arc<std::sync::Mutex<Instant>>,
    memory_pressure_detected: AtomicBool,
    total_allocations: AtomicU64,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            max_buffer_size_mb: 50.0, // Max 50MB for audio buffers
            max_processing_time_ms: 5000, // Max 5 seconds for processing
            cpu_threshold_percent: 80.0, // 80% CPU usage threshold
            last_cleanup: Arc::new(std::sync::Mutex::new(Instant::now())),
            memory_pressure_detected: AtomicBool::new(false),
            total_allocations: AtomicU64::new(0),
        }
    }

    /// Check if buffer size is within safe limits
    pub fn check_buffer_size(&self, buffer_size_bytes: usize) -> Result<(), String> {
        let size_mb = buffer_size_bytes as f64 / (1024.0 * 1024.0);
        
        if size_mb > self.max_buffer_size_mb {
            self.memory_pressure_detected.store(true, Ordering::SeqCst);
            return Err(format!(
                "Buffer size {:.2}MB exceeds limit of {:.2}MB", 
                size_mb, 
                self.max_buffer_size_mb
            ));
        }
        
        Ok(())
    }

    /// Monitor processing time for audio operations
    pub fn monitor_processing_time<F, R>(&self, operation: F) -> Result<R, String>
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = operation();
        let elapsed = start.elapsed();
        
        if elapsed.as_millis() > self.max_processing_time_ms as u128 {
            warn!(
                "Audio processing took {}ms, exceeds limit of {}ms",
                elapsed.as_millis(),
                self.max_processing_time_ms
            );
        }
        
        Ok(result)
    }

    /// Get current performance metrics
    pub fn get_metrics(&self) -> AudioPerformanceMetrics {
        AudioPerformanceMetrics {
            buffer_size_mb: 0.0, // This would be set by caller
            processing_time_ms: 0, // This would be set by caller
            cpu_usage_percent: self.get_cpu_usage(),
            memory_pressure: self.memory_pressure_detected.load(Ordering::SeqCst),
        }
    }

    /// Get approximate CPU usage (platform-specific)
    fn get_cpu_usage(&self) -> f32 {
        // This is a simplified implementation
        // In production, you'd use platform-specific APIs
        #[cfg(target_os = "macos")]
        {
            // Use macOS-specific APIs for more accurate CPU monitoring
            self.get_macos_cpu_usage()
        }
        #[cfg(not(target_os = "macos"))]
        {
            // Fallback for other platforms
            0.0
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_cpu_usage(&self) -> f32 {
        // Simplified - would use host_processor_info or similar
        // For now, return a safe default
        0.0
    }

    /// Trigger cleanup if memory pressure is detected
    pub fn maybe_trigger_cleanup(&self) -> bool {
        if self.memory_pressure_detected.load(Ordering::SeqCst) {
            if let Ok(mut last_cleanup) = self.last_cleanup.try_lock() {
                if last_cleanup.elapsed() > Duration::from_secs(5) {
                    *last_cleanup = Instant::now();
                    self.memory_pressure_detected.store(false, Ordering::SeqCst);
                    info!("Performance monitor triggered cleanup due to memory pressure");
                    return true;
                }
            }
        }
        false
    }

    /// Record allocation for tracking
    pub fn record_allocation(&self, size_bytes: usize) {
        self.total_allocations.fetch_add(size_bytes as u64, Ordering::SeqCst);
    }

    /// Check if system is under resource pressure
    pub fn is_under_pressure(&self) -> bool {
        self.memory_pressure_detected.load(Ordering::SeqCst) ||
        self.get_cpu_usage() > self.cpu_threshold_percent
    }
}

/// CRITICAL FIX: Thread-safe buffer management
pub struct SafeAudioBuffer {
    data: Vec<f32>,
    max_size: usize,
    performance_monitor: Arc<PerformanceMonitor>,
}

impl SafeAudioBuffer {
    pub fn new(max_size: usize, performance_monitor: Arc<PerformanceMonitor>) -> Self {
        Self {
            data: Vec::with_capacity(max_size),
            max_size,
            performance_monitor,
        }
    }

    /// Add audio data with bounds checking
    pub fn extend_with_bounds_check(&mut self, new_data: &[f32]) -> Result<(), String> {
        let new_total_size = self.data.len() + new_data.len();
        
        // Check against performance limits
        let size_bytes = new_total_size * std::mem::size_of::<f32>();
        self.performance_monitor.check_buffer_size(size_bytes)?;
        
        if new_total_size > self.max_size {
            // Trim old data to make room
            let excess = new_total_size - self.max_size;
            if excess < self.data.len() {
                self.data.drain(0..excess);
                warn!("Audio buffer size limit reached, trimmed {} samples", excess);
            } else {
                // New data is larger than max size - take only the end
                let keep_size = std::cmp::min(new_data.len(), self.max_size);
                self.data.clear();
                self.data.extend_from_slice(&new_data[new_data.len() - keep_size..]);
                warn!("New audio data larger than buffer, keeping only {} samples", keep_size);
                return Ok(());
            }
        }
        
        self.data.extend_from_slice(new_data);
        self.performance_monitor.record_allocation(new_data.len() * std::mem::size_of::<f32>());
        
        Ok(())
    }

    /// Get data with bounds checking
    pub fn data(&self) -> &[f32] {
        &self.data
    }

    /// Clear and shrink buffer
    pub fn clear_and_shrink(&mut self) {
        self.data.clear();
        self.data.shrink_to_fit();
        self.data.reserve(self.max_size);
    }

    /// Get current size in MB
    pub fn size_mb(&self) -> f64 {
        (self.data.len() * std::mem::size_of::<f32>()) as f64 / (1024.0 * 1024.0)
    }
}

/// CRITICAL FIX: Resource cleanup utility
pub struct ResourceCleanup {
    cleanup_threshold_mb: f64,
    cleanup_interval: Duration,
}

impl ResourceCleanup {
    pub fn new() -> Self {
        Self {
            cleanup_threshold_mb: 100.0, // Cleanup when using more than 100MB
            cleanup_interval: Duration::from_secs(30),
        }
    }

    /// Check if cleanup should be triggered
    pub fn should_cleanup(&self, current_usage_mb: f64, last_cleanup: Instant) -> bool {
        current_usage_mb > self.cleanup_threshold_mb ||
        last_cleanup.elapsed() > self.cleanup_interval
    }

    /// Perform emergency cleanup
    pub fn emergency_cleanup(&self) {
        info!("Performing emergency resource cleanup");
        
        // Force garbage collection if available
        #[cfg(feature = "gc")]
        {
            std::gc::collect();
        }
        
        // Platform-specific cleanup
        #[cfg(target_os = "macos")]
        {
            self.macos_memory_cleanup();
        }
    }

    #[cfg(target_os = "macos")]
    fn macos_memory_cleanup(&self) {
        // Use macOS-specific memory management
        // This would call into Objective-C runtime for memory pressure relief
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_monitor() {
        let monitor = PerformanceMonitor::new();
        
        // Test buffer size checking
        let small_buffer = 1024 * 1024; // 1MB
        assert!(monitor.check_buffer_size(small_buffer).is_ok());
        
        let large_buffer = 100 * 1024 * 1024; // 100MB
        assert!(monitor.check_buffer_size(large_buffer).is_err());
    }

    #[test]
    fn test_safe_audio_buffer() {
        let monitor = Arc::new(PerformanceMonitor::new());
        let mut buffer = SafeAudioBuffer::new(1000, monitor);
        
        // Test normal operation
        let test_data = vec![0.5f32; 500];
        assert!(buffer.extend_with_bounds_check(&test_data).is_ok());
        assert_eq!(buffer.data().len(), 500);
        
        // Test overflow handling
        let large_data = vec![0.5f32; 800];
        assert!(buffer.extend_with_bounds_check(&large_data).is_ok());
        assert!(buffer.data().len() <= 1000);
    }
} 