//! Real-time Performance Monitoring System
//!
//! TARS Phase 3.6.4: Performance benchmarking and metrics
//!
//! Advanced performance monitoring with real-time metrics collection,
//! alerting, and automated performance regression detection.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, Mutex as TokioMutex};
use tokio::time::interval;
use tracing::{debug, info, warn, error};

use crate::agent::memory::performance::{PerformanceMetrics, PerformanceSummary};

/// Real-time performance monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// Sampling interval for metrics collection
    pub sample_interval: Duration,
    /// Number of samples to keep in rolling window
    pub window_size: usize,
    /// Enable automatic alerting for performance regressions
    pub enable_alerting: bool,
    /// Threshold for performance regression detection (percentage)
    pub regression_threshold: f64,
    /// Minimum samples required before regression detection
    pub min_samples_for_detection: usize,
    /// Enable detailed memory tracking
    pub track_memory_details: bool,
    /// Enable latency histogram collection
    pub collect_latency_histograms: bool,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            sample_interval: Duration::from_secs(1),
            window_size: 300, // 5 minutes at 1-second intervals
            enable_alerting: true,
            regression_threshold: 20.0, // 20% performance degradation
            min_samples_for_detection: 30,
            track_memory_details: true,
            collect_latency_histograms: true,
        }
    }
}

/// Performance sample representing a point-in-time measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSample {
    pub timestamp: u64,
    pub throughput_ops_sec: f64,
    pub avg_latency_ms: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub error_rate: f64,
    pub cache_hit_rate: f64,
    pub pool_utilization: f64,
    pub concurrent_operations: u32,
}

/// Performance trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTrend {
    pub metric_name: String,
    pub trend_direction: TrendDirection,
    pub change_percent: f64,
    pub significance_level: f64,
    pub sample_count: usize,
    pub time_period: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Degrading,
    Stable,
    Insufficient,
}

/// Performance alert triggered when thresholds are exceeded
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAlert {
    pub alert_type: AlertType,
    pub metric_name: String,
    pub current_value: f64,
    pub threshold_value: f64,
    pub severity: AlertSeverity,
    pub timestamp: u64,
    pub description: String,
    pub suggested_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    Regression,
    MemoryLeak,
    HighLatency,
    LowThroughput,
    HighErrorRate,
    ResourceExhaustion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Latency histogram for detailed latency distribution analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyHistogram {
    pub buckets: Vec<LatencyBucket>,
    pub total_samples: u64,
    pub min_latency_ms: f64,
    pub max_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyBucket {
    pub upper_bound_ms: f64,
    pub count: u64,
    pub percentage: f64,
}

/// Real-time performance monitor
pub struct PerformanceMonitor {
    config: MonitorConfig,
    samples: Arc<RwLock<VecDeque<PerformanceSample>>>,
    alerts: Arc<RwLock<Vec<PerformanceAlert>>>,
    trends: Arc<RwLock<HashMap<String, PerformanceTrend>>>,
    latency_histogram: Arc<RwLock<Option<LatencyHistogram>>>,
    is_running: Arc<TokioMutex<bool>>,
    performance_metrics: Arc<PerformanceMetrics>,
}

impl PerformanceMonitor {
    pub fn new(config: MonitorConfig, performance_metrics: Arc<PerformanceMetrics>) -> Self {
        let window_size = config.window_size;
        Self {
            config,
            samples: Arc::new(RwLock::new(VecDeque::with_capacity(window_size))),
            alerts: Arc::new(RwLock::new(Vec::new())),
            trends: Arc::new(RwLock::new(HashMap::new())),
            latency_histogram: Arc::new(RwLock::new(None)),
            is_running: Arc::new(TokioMutex::new(false)),
            performance_metrics,
        }
    }

    /// Start the performance monitoring loop
    pub async fn start(&self) -> Result<(), String> {
        let mut running = self.is_running.lock().await;
        if *running {
            return Ok(()); // Already running
        }
        *running = true;
        
        info!("Starting performance monitor with {:?} sample interval", self.config.sample_interval);
        
        let samples = self.samples.clone();
        let alerts = self.alerts.clone();
        let trends = self.trends.clone();
        let latency_histogram = self.latency_histogram.clone();
        let performance_metrics = self.performance_metrics.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut interval = interval(config.sample_interval);
            
            while *is_running.lock().await {
                interval.tick().await;
                
                // Collect performance sample
                if let Ok(sample) = Self::collect_sample(&performance_metrics).await {
                    // Add to rolling window
                    {
                        let mut samples_guard = samples.write().await;
                        if samples_guard.len() >= config.window_size {
                            samples_guard.pop_front();
                        }
                        samples_guard.push_back(sample.clone());
                    }
                    
                    // Analyze trends
                    if config.enable_alerting {
                        let detected_trends = Self::analyze_trends(&samples, &config).await;
                        
                        // Update trends
                        {
                            let mut trends_guard = trends.write().await;
                            for trend in detected_trends {
                                trends_guard.insert(trend.metric_name.clone(), trend);
                            }
                        }
                        
                        // Check for alerts
                        let new_alerts = Self::check_alerts(&samples, &trends, &config).await;
                        if !new_alerts.is_empty() {
                            let mut alerts_guard = alerts.write().await;
                            alerts_guard.extend(new_alerts);
                            
                            // Keep only recent alerts (last hour)
                            let cutoff_time = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs() - 3600;
                            alerts_guard.retain(|alert| alert.timestamp > cutoff_time);
                        }
                    }
                    
                    // Update latency histogram
                    if config.collect_latency_histograms {
                        let histogram = Self::update_latency_histogram(&samples).await;
                        *latency_histogram.write().await = Some(histogram);
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the performance monitoring
    pub async fn stop(&self) {
        let mut running = self.is_running.lock().await;
        *running = false;
        info!("Performance monitor stopped");
    }

    /// Get current performance summary
    pub async fn get_current_summary(&self) -> PerformanceMonitorSummary {
        let samples = self.samples.read().await;
        let alerts = self.alerts.read().await;
        let trends = self.trends.read().await;
        let latency_histogram = self.latency_histogram.read().await;

        let current_sample = samples.back().cloned();
        let sample_count = samples.len();
        
        // Calculate averages over the window
        let avg_throughput = if !samples.is_empty() {
            samples.iter().map(|s| s.throughput_ops_sec).sum::<f64>() / samples.len() as f64
        } else {
            0.0
        };

        let avg_latency = if !samples.is_empty() {
            samples.iter().map(|s| s.avg_latency_ms).sum::<f64>() / samples.len() as f64
        } else {
            0.0
        };

        PerformanceMonitorSummary {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            window_size: self.config.window_size,
            sample_count,
            current_sample,
            avg_throughput_ops_sec: avg_throughput,
            avg_latency_ms: avg_latency,
            active_alerts: alerts.len(),
            recent_trends: trends.values().cloned().collect(),
            latency_histogram: latency_histogram.clone(),
        }
    }

    /// Get recent alerts
    pub async fn get_recent_alerts(&self, limit: Option<usize>) -> Vec<PerformanceAlert> {
        let alerts = self.alerts.read().await;
        let limit = limit.unwrap_or(alerts.len());
        
        alerts.iter()
            .rev() // Most recent first
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get performance trends for specific metrics
    pub async fn get_trends(&self) -> HashMap<String, PerformanceTrend> {
        self.trends.read().await.clone()
    }

    /// Export historical data for analysis
    pub async fn export_data(&self, start_time: Option<u64>, end_time: Option<u64>) -> PerformanceDataExport {
        let samples = self.samples.read().await;
        let alerts = self.alerts.read().await;
        
        let filtered_samples: Vec<PerformanceSample> = samples.iter()
            .filter(|sample| {
                if let Some(start) = start_time {
                    if sample.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = end_time {
                    if sample.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        let filtered_alerts: Vec<PerformanceAlert> = alerts.iter()
            .filter(|alert| {
                if let Some(start) = start_time {
                    if alert.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = end_time {
                    if alert.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        PerformanceDataExport {
            export_timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            start_time,
            end_time,
            samples: filtered_samples,
            alerts: filtered_alerts,
            config: self.config.clone(),
        }
    }

    /// Collect a single performance sample
    async fn collect_sample(metrics: &Arc<PerformanceMetrics>) -> Result<PerformanceSample, String> {
        let summary = metrics.get_summary();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        // Calculate derived metrics
        let throughput = if summary.avg_processing_time_us > 0 {
            1_000_000.0 / summary.avg_processing_time_us as f64 // ops per second
        } else {
            0.0
        };

        let error_rate = if summary.total_events > 0 {
            // This would need to be tracked separately in PerformanceMetrics
            0.0 // Placeholder
        } else {
            0.0
        };

        Ok(PerformanceSample {
            timestamp,
            throughput_ops_sec: throughput,
            avg_latency_ms: summary.avg_processing_time_us as f64 / 1000.0,
            memory_usage_mb: summary.current_memory_mb as f64,
            cpu_usage_percent: Self::get_cpu_usage().await,
            error_rate,
            cache_hit_rate: summary.cache_hit_rate,
            pool_utilization: summary.pool_hit_rate,
            concurrent_operations: summary.lock_contentions as u32,
        })
    }

    /// Get current CPU usage (simplified implementation)
    async fn get_cpu_usage() -> f64 {
        // This would need platform-specific implementation
        // For now, return a placeholder value
        0.0
    }

    /// Analyze performance trends
    async fn analyze_trends(
        samples: &Arc<RwLock<VecDeque<PerformanceSample>>>,
        config: &MonitorConfig,
    ) -> Vec<PerformanceTrend> {
        let samples_guard = samples.read().await;
        
        if samples_guard.len() < config.min_samples_for_detection {
            return vec![];
        }

        let samples_vec: Vec<&PerformanceSample> = samples_guard.iter().collect();
        let mut trends = Vec::new();

        // Analyze throughput trend
        if let Some(throughput_trend) = Self::analyze_metric_trend(
            "throughput",
            &samples_vec,
            |s| s.throughput_ops_sec,
            config,
        ) {
            trends.push(throughput_trend);
        }

        // Analyze latency trend
        if let Some(latency_trend) = Self::analyze_metric_trend(
            "latency",
            &samples_vec,
            |s| s.avg_latency_ms,
            config,
        ) {
            trends.push(latency_trend);
        }

        // Analyze memory trend
        if let Some(memory_trend) = Self::analyze_metric_trend(
            "memory",
            &samples_vec,
            |s| s.memory_usage_mb,
            config,
        ) {
            trends.push(memory_trend);
        }

        trends
    }

    /// Analyze trend for a specific metric
    fn analyze_metric_trend<F>(
        metric_name: &str,
        samples: &[&PerformanceSample],
        extractor: F,
        config: &MonitorConfig,
    ) -> Option<PerformanceTrend>
    where
        F: Fn(&PerformanceSample) -> f64,
    {
        if samples.len() < config.min_samples_for_detection {
            return None;
        }

        let values: Vec<f64> = samples.iter().map(|s| extractor(s)).collect();
        
        // Simple linear regression to detect trend
        let n = values.len() as f64;
        let x_mean = (n - 1.0) / 2.0; // Time is just index
        let y_mean = values.iter().sum::<f64>() / n;
        
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        
        for (i, &y) in values.iter().enumerate() {
            let x = i as f64;
            numerator += (x - x_mean) * (y - y_mean);
            denominator += (x - x_mean).powi(2);
        }
        
        if denominator == 0.0 {
            return None;
        }
        
        let slope = numerator / denominator;
        let first_value = values[0];
        let last_value = values[values.len() - 1];
        
        let change_percent = if first_value != 0.0 {
            ((last_value - first_value) / first_value) * 100.0
        } else {
            0.0
        };

        let trend_direction = if change_percent.abs() < 5.0 {
            TrendDirection::Stable
        } else if slope > 0.0 {
            if metric_name == "latency" || metric_name == "memory" {
                TrendDirection::Degrading // Higher latency/memory is worse
            } else {
                TrendDirection::Improving // Higher throughput is better
            }
        } else {
            if metric_name == "latency" || metric_name == "memory" {
                TrendDirection::Improving // Lower latency/memory is better
            } else {
                TrendDirection::Degrading // Lower throughput is worse
            }
        };

        // Calculate significance (simplified)
        let significance_level = change_percent.abs() / 100.0;

        Some(PerformanceTrend {
            metric_name: metric_name.to_string(),
            trend_direction,
            change_percent,
            significance_level,
            sample_count: samples.len(),
            time_period: Duration::from_secs(
                (samples.len() as u64) * config.sample_interval.as_secs()
            ),
        })
    }

    /// Check for performance alerts
    async fn check_alerts(
        samples: &Arc<RwLock<VecDeque<PerformanceSample>>>,
        trends: &Arc<RwLock<HashMap<String, PerformanceTrend>>>,
        config: &MonitorConfig,
    ) -> Vec<PerformanceAlert> {
        let samples_guard = samples.read().await;
        let trends_guard = trends.read().await;
        let mut alerts = Vec::new();
        
        if let Some(current_sample) = samples_guard.back() {
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

            // Check for performance regressions
            for (metric_name, trend) in trends_guard.iter() {
                if matches!(trend.trend_direction, TrendDirection::Degrading)
                    && trend.change_percent.abs() > config.regression_threshold
                {
                    alerts.push(PerformanceAlert {
                        alert_type: AlertType::Regression,
                        metric_name: metric_name.clone(),
                        current_value: match metric_name.as_str() {
                            "throughput" => current_sample.throughput_ops_sec,
                            "latency" => current_sample.avg_latency_ms,
                            "memory" => current_sample.memory_usage_mb,
                            _ => 0.0,
                        },
                        threshold_value: config.regression_threshold,
                        severity: if trend.change_percent.abs() > 50.0 {
                            AlertSeverity::Critical
                        } else if trend.change_percent.abs() > 30.0 {
                            AlertSeverity::Warning
                        } else {
                            AlertSeverity::Info
                        },
                        timestamp,
                        description: format!(
                            "{} has degraded by {:.1}% over the last {} samples",
                            metric_name, trend.change_percent.abs(), trend.sample_count
                        ),
                        suggested_actions: Self::get_suggested_actions(&metric_name),
                    });
                }
            }

            // Check for high error rate
            if current_sample.error_rate > 0.05 { // 5% error rate
                alerts.push(PerformanceAlert {
                    alert_type: AlertType::HighErrorRate,
                    metric_name: "error_rate".to_string(),
                    current_value: current_sample.error_rate * 100.0,
                    threshold_value: 5.0,
                    severity: AlertSeverity::Warning,
                    timestamp,
                    description: format!("High error rate detected: {:.1}%", current_sample.error_rate * 100.0),
                    suggested_actions: vec![
                        "Check application logs for error patterns".to_string(),
                        "Review recent code changes".to_string(),
                        "Monitor system resources".to_string(),
                    ],
                });
            }

            // Check for high latency
            if current_sample.avg_latency_ms > 1000.0 { // 1 second
                alerts.push(PerformanceAlert {
                    alert_type: AlertType::HighLatency,
                    metric_name: "latency".to_string(),
                    current_value: current_sample.avg_latency_ms,
                    threshold_value: 1000.0,
                    severity: AlertSeverity::Warning,
                    timestamp,
                    description: format!("High latency detected: {:.1}ms", current_sample.avg_latency_ms),
                    suggested_actions: Self::get_suggested_actions("latency"),
                });
            }
        }

        alerts
    }

    /// Get suggested actions for different types of performance issues
    fn get_suggested_actions(metric_name: &str) -> Vec<String> {
        match metric_name {
            "throughput" => vec![
                "Check for resource bottlenecks (CPU, memory, I/O)".to_string(),
                "Review concurrent processing settings".to_string(),
                "Optimize batch processing configuration".to_string(),
                "Consider scaling horizontally".to_string(),
            ],
            "latency" => vec![
                "Enable object pooling if disabled".to_string(),
                "Optimize cache configuration".to_string(),
                "Reduce batch sizes for lower latency".to_string(),
                "Check for lock contention".to_string(),
            ],
            "memory" => vec![
                "Check for memory leaks".to_string(),
                "Optimize cache sizes".to_string(),
                "Enable memory mapping for large datasets".to_string(),
                "Review object pool configurations".to_string(),
            ],
            _ => vec![
                "Check system resources".to_string(),
                "Review recent configuration changes".to_string(),
                "Monitor for external dependencies".to_string(),
            ],
        }
    }

    /// Update latency histogram
    async fn update_latency_histogram(
        samples: &Arc<RwLock<VecDeque<PerformanceSample>>>,
    ) -> LatencyHistogram {
        let samples_guard = samples.read().await;
        let latencies: Vec<f64> = samples_guard.iter().map(|s| s.avg_latency_ms).collect();
        
        if latencies.is_empty() {
            return LatencyHistogram {
                buckets: vec![],
                total_samples: 0,
                min_latency_ms: 0.0,
                max_latency_ms: 0.0,
            };
        }

        let min_latency = latencies.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_latency = latencies.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        // Create buckets: 0-1ms, 1-10ms, 10-100ms, 100-1000ms, 1000ms+
        let bucket_bounds = vec![1.0, 10.0, 100.0, 1000.0, f64::INFINITY];
        let mut buckets = Vec::new();
        
        for &upper_bound in &bucket_bounds {
            let count = latencies.iter()
                .filter(|&&latency| latency <= upper_bound)
                .count() as u64;
            
            let percentage = (count as f64 / latencies.len() as f64) * 100.0;
            
            buckets.push(LatencyBucket {
                upper_bound_ms: upper_bound,
                count,
                percentage,
            });
        }

        LatencyHistogram {
            buckets,
            total_samples: latencies.len() as u64,
            min_latency_ms: min_latency,
            max_latency_ms: max_latency,
        }
    }
}

/// Summary of current performance monitoring state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMonitorSummary {
    pub timestamp: u64,
    pub window_size: usize,
    pub sample_count: usize,
    pub current_sample: Option<PerformanceSample>,
    pub avg_throughput_ops_sec: f64,
    pub avg_latency_ms: f64,
    pub active_alerts: usize,
    pub recent_trends: Vec<PerformanceTrend>,
    pub latency_histogram: Option<LatencyHistogram>,
}

/// Exported performance data for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceDataExport {
    pub export_timestamp: u64,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub samples: Vec<PerformanceSample>,
    pub alerts: Vec<PerformanceAlert>,
    pub config: MonitorConfig,
}

impl PerformanceDataExport {
    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        let json_data = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize data: {}", e))?;
        
        std::fs::write(path, json_data)
            .map_err(|e| format!("Failed to write data to {}: {}", path, e))?;
        
        info!("Performance data exported to: {}", path);
        Ok(())
    }
}