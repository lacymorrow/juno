// Local Model Providers

pub mod ollama;
pub mod huggingface;

// Re-export for easy access
pub use ollama::OllamaModel;
pub use huggingface::HuggingFaceModel;