/// LLM-to-LLM Quality Assurance and Calibration System
/// 
/// This module implements a sophisticated QA system where one LLM agent tests
/// and coordinates with another LLM agent for enhanced quality assurance.

pub mod coordinator;

// Re-export key types for easy access
pub use coordinator::*;