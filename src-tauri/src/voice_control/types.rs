//! Types and configuration for voice control functionality
//! 
//! This module centralizes all voice control related types to reduce duplication
//! and provide clear interfaces between modules.

use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::sync::{Arc, Mutex};

/// Messages that can be sent to the audio capture thread
#[derive(Debug, Clone)]
pub enum AudioThreadMessage {
    /// Stop audio capture and transcription
    Stop,
    /// Pause audio capture temporarily
    Pause,
    /// Resume audio capture
    Resume,
    /// Update configuration
    UpdateConfig(VoiceControllerConfig),
}

/// Configuration for voice controller behavior
#[derive(Debug, Clone)]
pub struct VoiceControllerConfig {
    /// Sample rate for audio capture (default: device native)
    pub capture_sample_rate: Option<u32>,
    
    /// Target sample rate for Whisper (should be 16000)
    pub whisper_sample_rate: u32,
    
    /// Buffer duration for partial transcriptions (milliseconds)
    pub partial_buffer_duration_ms: u64,
    
    /// Buffer duration for final transcription (milliseconds)
    pub final_buffer_duration_ms: u64,
    
    /// Enable developer playback features
    pub developer_playback_enabled: bool,
    
    /// Number of threads for Whisper processing
    pub whisper_threads: i32,
    
    /// Enable real-time partial results
    pub enable_partial_results: bool,
    
    /// Minimum audio length for transcription (milliseconds)
    pub min_audio_length_ms: u64,
}

impl Default for VoiceControllerConfig {
    fn default() -> Self {
        Self {
            capture_sample_rate: None, // Use device default
            whisper_sample_rate: 16000,
            partial_buffer_duration_ms: 1500,
            final_buffer_duration_ms: 5000,
            developer_playback_enabled: false,
            whisper_threads: 4,
            enable_partial_results: true,
            min_audio_length_ms: 500,
        }
    }
}

/// Result from transcription operations
#[derive(Debug, Clone, serde::Serialize)]
pub struct TranscriptionResult {
    /// The transcribed text
    pub text: String,
    
    /// Whether this is a partial result (may change) or final
    pub is_partial: bool,
    
    /// Confidence score if available (0.0 to 1.0)
    pub confidence: Option<f32>,
    
    /// Processing time in milliseconds
    pub processing_time_ms: f64,
    
    /// Length of audio processed (seconds)
    pub audio_duration_seconds: f32,
}

/// Audio capture state and statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioCaptureState {
    /// Whether audio capture is currently active
    pub is_capturing: bool,
    
    /// Whether transcription is currently processing
    pub is_processing: bool,
    
    /// Current audio buffer size (number of samples)
    pub buffer_size: usize,
    
    /// Actual sample rate being used for capture
    pub actual_sample_rate: Option<u32>,
    
    /// Total samples captured in current session
    pub total_samples_captured: u64,
    
    /// Number of transcription operations completed
    pub transcription_count: u32,
    
    /// Average processing time per transcription (milliseconds)
    pub avg_processing_time_ms: f64,
}

impl Default for AudioCaptureState {
    fn default() -> Self {
        Self {
            is_capturing: false,
            is_processing: false,
            buffer_size: 0,
            actual_sample_rate: None,
            total_samples_captured: 0,
            transcription_count: 0,
            avg_processing_time_ms: 0.0,
        }
    }
}

/// Handle for managing an audio capture thread
pub struct AudioThreadHandle {
    /// Join handle for the thread
    pub handle: JoinHandle<()>,
    
    /// Sender for sending messages to the thread
    pub sender: Sender<AudioThreadMessage>,
}

impl AudioThreadHandle {
    /// Create a new audio thread handle
    pub fn new(handle: JoinHandle<()>, sender: Sender<AudioThreadMessage>) -> Self {
        Self { handle, sender }
    }
    
    /// Send a message to the audio thread
    pub fn send_message(&self, message: AudioThreadMessage) -> Result<(), String> {
        self.sender.send(message)
            .map_err(|e| format!("Failed to send message to audio thread: {}", e))
    }
    
    /// Stop the audio thread
    pub fn stop(&self) -> Result<(), String> {
        self.send_message(AudioThreadMessage::Stop)
    }
    
    /// Pause audio capture
    pub fn pause(&self) -> Result<(), String> {
        self.send_message(AudioThreadMessage::Pause)
    }
    
    /// Resume audio capture
    pub fn resume(&self) -> Result<(), String> {
        self.send_message(AudioThreadMessage::Resume)
    }
}

/// Audio buffer with metadata
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// Raw audio samples (f32, mono)
    pub samples: Vec<f32>,
    
    /// Sample rate of the audio
    pub sample_rate: u32,
    
    /// Timestamp when this buffer was captured
    pub timestamp_ms: u64,
    
    /// Duration of the audio in seconds
    pub duration_seconds: f32,
}

impl AudioBuffer {
    /// Create a new audio buffer
    pub fn new(samples: Vec<f32>, sample_rate: u32, timestamp_ms: u64) -> Self {
        let duration_seconds = samples.len() as f32 / sample_rate as f32;
        Self {
            samples,
            sample_rate,
            timestamp_ms,
            duration_seconds,
        }
    }
    
    /// Get the number of samples in this buffer
    pub fn len(&self) -> usize {
        self.samples.len()
    }
    
    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
    
    /// Get the duration in milliseconds
    pub fn duration_ms(&self) -> f64 {
        self.duration_seconds as f64 * 1000.0
    }
}

/// Error types for voice control operations
#[derive(Debug, thiserror::Error)]
pub enum VoiceControlError {
    #[error("Model file error: {0}")]
    ModelFile(String),
    
    #[error("Audio capture error: {0}")]
    AudioCapture(String),
    
    #[error("Transcription error: {0}")]
    Transcription(String),
    
    #[error("Resampling error: {0}")]
    Resampling(String),
    
    #[error("Thread communication error: {0}")]
    ThreadCommunication(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
}

/// Convert VoiceControlError to String for Tauri compatibility
impl From<VoiceControlError> for String {
    fn from(error: VoiceControlError) -> Self {
        error.to_string()
    }
}