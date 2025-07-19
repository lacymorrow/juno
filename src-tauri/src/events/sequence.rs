//! # Event Sequence Management
//!
//! Provides sequence numbers for events to ensure ordering and prevent
//! race conditions in event processing.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::collections::HashMap;
use parking_lot::RwLock;
use serde::{Serialize, Deserialize};
use tauri::{AppHandle, Emitter, Manager};

/// Global sequence number generator
static GLOBAL_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Get the next global sequence number
pub fn next_sequence() -> u64 {
    GLOBAL_SEQUENCE.fetch_add(1, Ordering::SeqCst)
}

/// Event with sequence number
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequencedEvent<T: Clone> {
    /// Unique sequence number for ordering
    pub sequence: u64,
    /// Timestamp when event was created
    pub timestamp: u64,
    /// The actual event payload
    pub payload: T,
}

impl<T: Clone> SequencedEvent<T> {
    /// Create a new sequenced event
    pub fn new(payload: T) -> Self {
        Self {
            sequence: next_sequence(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            payload,
        }
    }
}

/// Event sequencer for a specific event type
#[derive(Debug)]
pub struct EventSequencer {
    /// Name of the event type
    event_name: String,
    /// Last sequence number emitted
    last_sequence: AtomicU64,
    /// Buffer for out-of-order events
    buffer: Arc<RwLock<HashMap<u64, serde_json::Value>>>,
}

impl EventSequencer {
    /// Create a new event sequencer
    pub fn new(event_name: String) -> Self {
        Self {
            event_name,
            last_sequence: AtomicU64::new(0),
            buffer: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Emit a sequenced event, ensuring order
    pub fn emit<T: Serialize + Clone>(&self, app_handle: &AppHandle, payload: T) -> Result<(), String> {
        let sequenced = SequencedEvent::new(payload);
        let sequence = sequenced.sequence;
        
        // Convert to JSON for emission
        let json_value = serde_json::to_value(&sequenced)
            .map_err(|e| format!("Failed to serialize event: {}", e))?;
        
        // Check if this is the next expected sequence
        let last = self.last_sequence.load(Ordering::SeqCst);
        
        if sequence == last + 1 {
            // This is the next expected event, emit it
            app_handle.emit(&self.event_name, &json_value)
                .map_err(|e| format!("Failed to emit event: {}", e))?;
            
            self.last_sequence.store(sequence, Ordering::SeqCst);
            
            // Check buffer for any events that can now be emitted
            self.flush_buffer(app_handle);
            
            Ok(())
        } else if sequence > last + 1 {
            // This event is out of order, buffer it
            let mut buffer = self.buffer.write();
            buffer.insert(sequence, json_value);
            Ok(())
        } else {
            // This is an old event, ignore it
            Err(format!("Event sequence {} is older than last emitted {}", sequence, last))
        }
    }
    
    /// Flush any buffered events that can now be emitted
    fn flush_buffer(&self, app_handle: &AppHandle) {
        let mut last = self.last_sequence.load(Ordering::SeqCst);
        let mut buffer = self.buffer.write();
        
        loop {
            let next = last + 1;
            if let Some(event) = buffer.remove(&next) {
                // Emit the buffered event
                if let Err(e) = app_handle.emit(&self.event_name, &event) {
                    tracing::error!("Failed to emit buffered event: {}", e);
                    break;
                }
                
                last = next;
                self.last_sequence.store(last, Ordering::SeqCst);
            } else {
                // No more sequential events in buffer
                break;
            }
        }
    }
    
    /// Get the current sequence number
    pub fn current_sequence(&self) -> u64 {
        self.last_sequence.load(Ordering::SeqCst)
    }
    
    /// Get the size of the buffer (for monitoring)
    pub fn buffer_size(&self) -> usize {
        self.buffer.read().len()
    }
}

/// Manager for all event sequencers
#[derive(Debug)]
pub struct SequenceManager {
    sequencers: Arc<RwLock<HashMap<String, Arc<EventSequencer>>>>,
}

impl SequenceManager {
    /// Create a new sequence manager
    pub fn new() -> Self {
        Self {
            sequencers: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Get or create a sequencer for an event type
    pub fn get_sequencer(&self, event_name: &str) -> Arc<EventSequencer> {
        let mut sequencers = self.sequencers.write();
        
        sequencers.entry(event_name.to_string())
            .or_insert_with(|| Arc::new(EventSequencer::new(event_name.to_string())))
            .clone()
    }
    
    /// Emit a sequenced event
    pub fn emit<T: Serialize + Clone>(
        &self,
        app_handle: &AppHandle,
        event_name: &str,
        payload: T,
    ) -> Result<(), String> {
        let sequencer = self.get_sequencer(event_name);
        sequencer.emit(app_handle, payload)
    }
    
    /// Get stats for all sequencers
    pub fn get_stats(&self) -> HashMap<String, (u64, usize)> {
        let sequencers = self.sequencers.read();
        
        sequencers.iter()
            .map(|(name, seq)| {
                (name.clone(), (seq.current_sequence(), seq.buffer_size()))
            })
            .collect()
    }
}

/// Extension trait for AppHandle to emit sequenced events
pub trait SequencedEmitter {
    /// Emit an event with automatic sequencing
    fn emit_sequenced<T: Serialize + Clone>(&self, event: &str, payload: T) -> Result<(), String>;
}

impl SequencedEmitter for AppHandle {
    fn emit_sequenced<T: Serialize + Clone>(&self, event: &str, payload: T) -> Result<(), String> {
        // Get the sequence manager from app state
        match self.try_state::<Arc<SequenceManager>>() {
            Some(manager) => manager.emit(self, event, payload),
            None => {
                // Fallback to regular emit with sequence number
                let sequenced = SequencedEvent::new(payload);
                self.emit(event, sequenced)
                    .map_err(|e| format!("Failed to emit event: {}", e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sequence_generation() {
        let seq1 = next_sequence();
        let seq2 = next_sequence();
        let seq3 = next_sequence();
        
        assert_eq!(seq2, seq1 + 1);
        assert_eq!(seq3, seq2 + 1);
    }
    
    #[test]
    fn test_sequenced_event() {
        let event1 = SequencedEvent::new("test1");
        let event2 = SequencedEvent::new("test2");
        
        assert!(event2.sequence > event1.sequence);
        assert!(event2.timestamp >= event1.timestamp);
    }
}