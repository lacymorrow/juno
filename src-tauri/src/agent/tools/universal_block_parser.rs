//! Universal Block Parsing (UBP) Implementation
//! Based on SpiritSight Agent research from CVPR 2025
//!
//! Solves the fundamental GUI element grounding problem by replacing global coordinates
//! with block-specific coordinates using 2D Block-wise Position Embedding.
//!
//! Used by: Anthropic Computer Use tools for enhanced GUI element grounding

use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use image::{ImageBuffer, Rgba};

/// Configuration for Universal Block Parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UBPConfig {
    /// Block size for grid division (e.g., 64x64 pixels)
    pub block_size: u32,
    /// Overlap between adjacent blocks (0.0 to 1.0)
    pub block_overlap: f32,
    /// Whether to use adaptive block sizing based on content density
    pub adaptive_sizing: bool,
    /// Minimum block size when using adaptive sizing
    pub min_block_size: u32,
    /// Maximum block size when using adaptive sizing
    pub max_block_size: u32,
    /// Confidence threshold for element detection within blocks
    pub detection_confidence: f32,
}

impl Default for UBPConfig {
    fn default() -> Self {
        Self {
            block_size: 64,
            block_overlap: 0.1,
            adaptive_sizing: true,
            min_block_size: 32,
            max_block_size: 128,
            detection_confidence: 0.7,
        }
    }
}

impl UBPConfig {
    /// Validates the configuration and returns an error if invalid
    pub fn validate(&self) -> Result<(), UBPError> {
        if self.block_size == 0 {
            return Err(UBPError::InvalidInput("block_size cannot be zero".to_string()));
        }
        if self.min_block_size == 0 {
            return Err(UBPError::InvalidInput("min_block_size cannot be zero".to_string()));
        }
        if self.max_block_size == 0 {
            return Err(UBPError::InvalidInput("max_block_size cannot be zero".to_string()));
        }
        if self.min_block_size > self.max_block_size {
            return Err(UBPError::InvalidInput("min_block_size cannot be greater than max_block_size".to_string()));
        }
        if self.block_overlap < 0.0 || self.block_overlap >= 1.0 {
            return Err(UBPError::InvalidInput("block_overlap must be between 0.0 and 1.0 (exclusive)".to_string()));
        }
        if self.detection_confidence < 0.0 || self.detection_confidence > 1.0 {
            return Err(UBPError::InvalidInput("detection_confidence must be between 0.0 and 1.0 (inclusive)".to_string()));
        }
        Ok(())
    }
}

/// Block-specific coordinates within a UBP grid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockCoordinates {
    /// Block index in the grid (row-major order)
    pub block_index: u32,
    /// Relative X coordinate within the block (0.0 to 1.0)
    pub relative_x: f32,
    /// Relative Y coordinate within the block (0.0 to 1.0)
    pub relative_y: f32,
    /// Confidence score for this coordinate mapping
    pub confidence: f32,
}

/// Individual block in the UBP grid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UBPBlock {
    /// Unique identifier for this block
    pub id: u32,
    /// Global pixel coordinates of block top-left corner
    pub global_x: u32,
    pub global_y: u32,
    /// Block dimensions in pixels
    pub width: u32,
    pub height: u32,
    /// 2D position embedding for spatial relationships
    pub position_embedding: Vec<f32>,
    /// Detected UI elements within this block
    pub elements: Vec<BlockElement>,
    /// Content density score (higher = more UI elements)
    pub density_score: f32,
}

/// UI element detected within a specific block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockElement {
    /// Element type (button, text_field, etc.)
    pub element_type: String,
    /// Relative coordinates within the block
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Confidence score for this detection
    pub confidence: f32,
    /// Semantic description of the element
    pub description: Option<String>,
}

/// Result of Universal Block Parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UBPResult {
    /// Original image dimensions
    pub image_width: u32,
    pub image_height: u32,
    /// Grid configuration used
    pub grid_config: UBPConfig,
    /// All blocks in the parsed grid
    pub blocks: Vec<UBPBlock>,
    /// Grid dimensions (blocks_x, blocks_y)
    pub grid_dimensions: (u32, u32),
    /// Actual block size used (may differ from config due to adaptive sizing)
    pub actual_block_size: u32,
    /// Total processing time in milliseconds
    pub processing_time_ms: u64,
    /// Number of UI elements detected across all blocks
    pub total_elements_detected: u32,
}

/// Error type for UBP operations
#[derive(Debug, thiserror::Error)]
pub enum UBPError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Processing error: {0}")]
    ProcessingError(String),
}

/// Main Universal Block Parser
pub struct UniversalBlockParser {
    config: UBPConfig,
}

impl UniversalBlockParser {
    /// Creates a new UBP parser with default configuration
    pub fn new() -> Self {
        Self {
            config: UBPConfig::default(),
        }
    }

    /// Creates a new UBP parser with custom configuration
    pub fn new_with_config(config: UBPConfig) -> Result<Self, UBPError> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Parses a screenshot into a UBP grid structure
    pub async fn parse_screenshot(
        &self,
        image_buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    ) -> Result<UBPResult, UBPError> {
        let start_time = std::time::Instant::now();

        info!("Starting UBP parsing for {}x{} image", image_buffer.width(), image_buffer.height());

        // Step 1: Create grid layout and get actual block size
        let (blocks_x, blocks_y, actual_block_size) = self.calculate_grid_dimensions_with_size(
            image_buffer.width(),
            image_buffer.height(),
        );

        debug!("UBP grid dimensions: {}x{} blocks with actual block size: {}", blocks_x, blocks_y, actual_block_size);

        // Step 2: Generate blocks with position embeddings
        let mut blocks = Vec::new();
        let mut total_elements = 0;

        for row in 0..blocks_y {
            for col in 0..blocks_x {
                let block_id = row * blocks_x + col;
                let block = self.create_block_with_size(
                    block_id,
                    col,
                    row,
                    actual_block_size,
                    image_buffer,
                ).await?;

                total_elements += block.elements.len() as u32;
                blocks.push(block);
            }
        }

        let processing_time = start_time.elapsed().as_millis() as u64;

        Ok(UBPResult {
            image_width: image_buffer.width(),
            image_height: image_buffer.height(),
            grid_config: self.config.clone(),
            blocks,
            grid_dimensions: (blocks_x, blocks_y),
            actual_block_size,
            processing_time_ms: processing_time,
            total_elements_detected: total_elements,
        })
    }

    /// Parses a screenshot from raw image bytes
    pub async fn parse_screenshot_from_bytes(
        &self,
        image_bytes: &[u8],
    ) -> Result<UBPResult, UBPError> {
        // Decode image bytes to ImageBuffer
        let image_buffer = image::load_from_memory(image_bytes)
            .map_err(|e| UBPError::ProcessingError(format!("Failed to decode image: {}", e)))?
            .to_rgba8();

        // Use the existing parse_screenshot method
        self.parse_screenshot(&image_buffer).await
    }

    /// Converts global coordinates to block-specific coordinates
    pub fn global_to_block_coordinates(
        &self,
        ubp_result: &UBPResult,
        global_x: f64,
        global_y: f64,
    ) -> Result<BlockCoordinates, UBPError> {
        let (blocks_x, blocks_y) = ubp_result.grid_dimensions;

        // Check for zero block size to prevent division by zero
        if ubp_result.actual_block_size == 0 {
            return Err(UBPError::InvalidInput(
                "Block size cannot be zero".to_string()
            ));
        }

        // Check for zero grid dimensions to prevent underflow
        if blocks_x == 0 || blocks_y == 0 {
            return Err(UBPError::InvalidInput(
                "Grid dimensions cannot be zero".to_string()
            ));
        }

        // Clamp coordinates to image boundaries to prevent out-of-bounds access
        // This handles both negative coordinates and coordinates beyond image bounds
        let clamped_x = global_x.min(ubp_result.image_width as f64 - 1.0).max(0.0);
        let clamped_y = global_y.min(ubp_result.image_height as f64 - 1.0).max(0.0);

        // Calculate block indices using clamped coordinates
        let block_col = (clamped_x as u32) / ubp_result.actual_block_size;
        let block_row = (clamped_y as u32) / ubp_result.actual_block_size;

        // Additional bounds checking for block indices (safe from underflow now)
        let block_col = block_col.min(blocks_x - 1);
        let block_row = block_row.min(blocks_y - 1);

        let block_index = block_row * blocks_x + block_col;

        // Ensure block_index is within bounds
        if block_index >= ubp_result.blocks.len() as u32 {
            return Err(UBPError::InvalidInput(format!(
                "Block index {} out of bounds for coordinates ({}, {}). Grid has {} blocks ({}x{}).",
                block_index, global_x, global_y, ubp_result.blocks.len(), blocks_x, blocks_y
            )));
        }

        if let Some(block) = ubp_result.blocks.get(block_index as usize) {
            // Calculate relative coordinates within the block using clamped coordinates
            // for consistent coordinate handling
            let block_relative_x = clamped_x - block.global_x as f64;
            let block_relative_y = clamped_y - block.global_y as f64;

            let relative_x = (block_relative_x / block.width as f64).max(0.0).min(1.0) as f32;
            let relative_y = (block_relative_y / block.height as f64).max(0.0).min(1.0) as f32;

            // Adjust confidence based on whether coordinates were clamped
            let confidence = if global_x != clamped_x || global_y != clamped_y {
                0.7 // Lower confidence for clamped coordinates
            } else {
                0.95 // High confidence for coordinates within bounds
            };

            Ok(BlockCoordinates {
                block_index,
                relative_x,
                relative_y,
                confidence,
            })
        } else {
            Err(UBPError::InvalidInput(format!(
                "No block found for coordinates ({}, {})", global_x, global_y
            )))
        }
    }

    /// Converts block-specific coordinates back to global coordinates
    pub fn block_to_global_coordinates(
        &self,
        ubp_result: &UBPResult,
        block_coords: &BlockCoordinates,
    ) -> Result<(f64, f64), UBPError> {
        if let Some(block) = ubp_result.blocks.get(block_coords.block_index as usize) {
            let global_x = block.global_x as f64 + (block_coords.relative_x as f64 * block.width as f64);
            let global_y = block.global_y as f64 + (block_coords.relative_y as f64 * block.height as f64);

            Ok((global_x, global_y))
        } else {
            Err(UBPError::InvalidInput(format!(
                "Invalid block index: {}", block_coords.block_index
            )))
        }
    }

    /// Calculate optimal grid dimensions based on image size and configuration
    #[allow(dead_code)]
    fn calculate_grid_dimensions(&self, width: u32, height: u32) -> (u32, u32) {
        let (blocks_x, blocks_y, _) = self.calculate_grid_dimensions_with_size(width, height);
        (blocks_x, blocks_y)
    }

    /// Calculate optimal grid dimensions and actual block size based on image size and configuration
    fn calculate_grid_dimensions_with_size(&self, width: u32, height: u32) -> (u32, u32, u32) {
        let block_size = if self.config.adaptive_sizing {
            // Use adaptive sizing based on image resolution
            let base_size = self.config.block_size;
            let scale_factor = ((width * height) as f32 / (1920.0 * 1080.0)).sqrt();

            (base_size as f32 * scale_factor)
                .max(self.config.min_block_size as f32)
                .min(self.config.max_block_size as f32) as u32
        } else {
            self.config.block_size
        };

        // Prevent division by zero by ensuring block_size is at least 1
        let safe_block_size = block_size.max(1);

        let blocks_x = (width + safe_block_size - 1) / safe_block_size; // Ceiling division
        let blocks_y = (height + safe_block_size - 1) / safe_block_size;

        (blocks_x, blocks_y, safe_block_size)
    }

    /// Creates a single block with position embedding and element detection
    #[allow(dead_code)]
    async fn create_block(
        &self,
        block_id: u32,
        col: u32,
        row: u32,
        image_buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    ) -> Result<UBPBlock, UBPError> {
        let block_size = self.config.block_size;
        self.create_block_with_size(block_id, col, row, block_size, image_buffer).await
    }

    /// Creates a single block with position embedding and element detection using specified block size
    async fn create_block_with_size(
        &self,
        block_id: u32,
        col: u32,
        row: u32,
        block_size: u32,
        image_buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    ) -> Result<UBPBlock, UBPError> {
        let global_x = col * block_size;
        let global_y = row * block_size;

        // Calculate actual block dimensions (may be smaller at image edges)
        let width = block_size.min(image_buffer.width() - global_x);
        let height = block_size.min(image_buffer.height() - global_y);

        // Generate 2D position embedding
        let position_embedding = self.generate_position_embedding(col, row, image_buffer.width(), image_buffer.height(), block_size);

        // Detect UI elements within this block
        let elements = self.detect_elements_in_block(
            image_buffer,
            global_x,
            global_y,
            width,
            height,
        ).await?;

        // Calculate content density
        let density_score = elements.len() as f32 / (width * height) as f32 * 10000.0; // Scale to reasonable range

        Ok(UBPBlock {
            id: block_id,
            global_x,
            global_y,
            width,
            height,
            position_embedding,
            elements,
            density_score,
        })
    }

    /// Generates 2D position embedding for spatial relationships
    fn generate_position_embedding(&self, col: u32, row: u32, image_width: u32, image_height: u32, actual_block_size: u32) -> Vec<f32> {
        let mut embedding = Vec::with_capacity(16); // 16-dimensional embedding

        // Calculate grid dimensions using actual block size to prevent division by zero
        let blocks_x = if actual_block_size > 0 { (image_width + actual_block_size - 1) / actual_block_size } else { 1 };
        let blocks_y = if actual_block_size > 0 { (image_height + actual_block_size - 1) / actual_block_size } else { 1 };

        // Ensure we don't divide by zero
        let blocks_x_f32 = blocks_x.max(1) as f32;
        let blocks_y_f32 = blocks_y.max(1) as f32;

        // Absolute position encoding (normalized to 0-1 range)
        embedding.push(col as f32 / blocks_x_f32);
        embedding.push(row as f32 / blocks_y_f32);

        // Relative position encoding (distance from corners and center)
        let center_x = blocks_x_f32 / 2.0;
        let center_y = blocks_y_f32 / 2.0;

        // Distance from center (normalized)
        let dist_from_center_x = if center_x > 0.0 { (col as f32 - center_x) / center_x } else { 0.0 };
        let dist_from_center_y = if center_y > 0.0 { (row as f32 - center_y) / center_y } else { 0.0 };
        embedding.push(dist_from_center_x);
        embedding.push(dist_from_center_y);

        // Corner distances (normalized)
        embedding.push(col as f32 / blocks_x_f32); // Distance from left (normalized)
        embedding.push(row as f32 / blocks_y_f32); // Distance from top (normalized)

        // Additional spatial features
        embedding.push((blocks_x - 1 - col) as f32 / blocks_x_f32); // Distance from right (normalized)
        embedding.push((blocks_y - 1 - row) as f32 / blocks_y_f32); // Distance from bottom (normalized)

        // Block size information
        embedding.push(actual_block_size as f32 / image_width.max(1) as f32); // Block size relative to width
        embedding.push(actual_block_size as f32 / image_height.max(1) as f32); // Block size relative to height

        // Pad to 16 dimensions
        while embedding.len() < 16 {
            embedding.push(0.0);
        }

        embedding
    }

    /// Detects UI elements within a specific block region
    async fn detect_elements_in_block(
        &self,
        image_buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        block_x: u32,
        block_y: u32,
        block_width: u32,
        block_height: u32,
    ) -> Result<Vec<BlockElement>, UBPError> {
        let mut elements = Vec::new();

        // Extract block region for analysis
        let block_region = self.extract_block_region(image_buffer, block_x, block_y, block_width, block_height)?;

        // Simple element detection based on visual patterns
        // In a production implementation, this would use a trained ML model
        elements.extend(self.detect_buttons(&block_region).await?);
        elements.extend(self.detect_text_fields(&block_region).await?);

        Ok(elements)
    }

    /// Extracts a specific block region from the full image
    fn extract_block_region(
        &self,
        image_buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        block_x: u32,
        block_y: u32,
        block_width: u32,
        block_height: u32,
    ) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, UBPError> {
        let mut block_buffer = ImageBuffer::new(block_width, block_height);

        for y in 0..block_height {
            for x in 0..block_width {
                let source_x = block_x + x;
                let source_y = block_y + y;

                if source_x < image_buffer.width() && source_y < image_buffer.height() {
                    let pixel = image_buffer.get_pixel(source_x, source_y);
                    block_buffer.put_pixel(x, y, *pixel);
                }
            }
        }

        Ok(block_buffer)
    }

    /// Detects button-like elements within a block
    async fn detect_buttons(&self, block_region: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<Vec<BlockElement>, UBPError> {
        let mut buttons = Vec::new();

        // Simple button detection using edge detection and color analysis
        let width = block_region.width();
        let height = block_region.height();

        // Grid-based scanning for button-like patterns
        for y in (0..height).step_by(8) {
            for x in (0..width).step_by(8) {
                if let Some(button) = self.analyze_potential_button(block_region, x, y, 32, 24) {
                    buttons.push(button);
                }
            }
        }

        Ok(buttons)
    }

    /// Detects text field elements within a block
    async fn detect_text_fields(&self, block_region: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<Vec<BlockElement>, UBPError> {
        let mut text_fields = Vec::new();

        // Simple text field detection using horizontal line patterns
        let width = block_region.width();
        let height = block_region.height();

        for y in (0..height).step_by(4) {
            for x in (0..width).step_by(8) {
                if let Some(text_field) = self.analyze_potential_text_field(block_region, x, y, 64, 20) {
                    text_fields.push(text_field);
                }
            }
        }

        Ok(text_fields)
    }

    /// Analyzes a region for button-like characteristics
    fn analyze_potential_button(
        &self,
        block_region: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Option<BlockElement> {
        // Ensure the region is within bounds
        if x + width > block_region.width() || y + height > block_region.height() {
            return None;
        }

        // Sample pixels to detect button-like patterns
        let mut edge_pixels = 0;
        let mut total_pixels = 0;
        let mut avg_brightness = 0.0;

        for dy in 0..height {
            for dx in 0..width {
                let pixel = block_region.get_pixel(x + dx, y + dy);
                let brightness = (pixel[0] as f32 + pixel[1] as f32 + pixel[2] as f32) / 3.0;
                avg_brightness += brightness;
                total_pixels += 1;

                // Check for edge characteristics (border pixels)
                if dx == 0 || dx == width - 1 || dy == 0 || dy == height - 1 {
                    if brightness > 200.0 || brightness < 50.0 {
                        edge_pixels += 1;
                    }
                }
            }
        }

        avg_brightness /= total_pixels as f32;

        // Simple heuristic: buttons often have distinct borders and uniform interiors
        let edge_ratio = edge_pixels as f32 / (2.0 * (width + height) as f32);
        let confidence = if edge_ratio > 0.3 && avg_brightness > 100.0 && avg_brightness < 200.0 {
            0.8
        } else if edge_ratio > 0.2 {
            0.6
        } else {
            0.3
        };

        if confidence > self.config.detection_confidence {
            Some(BlockElement {
                element_type: "button".to_string(),
                x: x as f32 / block_region.width() as f32,
                y: y as f32 / block_region.height() as f32,
                width: width as f32 / block_region.width() as f32,
                height: height as f32 / block_region.height() as f32,
                confidence,
                description: Some("Detected button element".to_string()),
            })
        } else {
            None
        }
    }

    /// Analyzes a region for text field characteristics
    fn analyze_potential_text_field(
        &self,
        block_region: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Option<BlockElement> {
        if x + width > block_region.width() || y + height > block_region.height() {
            return None;
        }

        // Look for horizontal line patterns typical of text fields
        let mut horizontal_lines = 0;
        let mut total_brightness = 0.0;
        let mut pixel_count = 0;

        for dy in 0..height {
            let mut line_brightness = 0.0;
            let mut line_pixels = 0;

            for dx in 0..width {
                let pixel = block_region.get_pixel(x + dx, y + dy);
                let brightness = (pixel[0] as f32 + pixel[1] as f32 + pixel[2] as f32) / 3.0;
                line_brightness += brightness;
                total_brightness += brightness;
                line_pixels += 1;
                pixel_count += 1;
            }

            line_brightness /= line_pixels as f32;

            // Check for horizontal line patterns (very bright or very dark lines)
            if line_brightness < 50.0 || line_brightness > 200.0 {
                horizontal_lines += 1;
            }
        }

        let avg_brightness = total_brightness / pixel_count as f32;
        let line_ratio = horizontal_lines as f32 / height as f32;

        let confidence = if line_ratio > 0.1 && avg_brightness > 180.0 {
            0.7
        } else if line_ratio > 0.05 {
            0.5
        } else {
            0.2
        };

        if confidence > self.config.detection_confidence {
            Some(BlockElement {
                element_type: "text_field".to_string(),
                x: x as f32 / block_region.width() as f32,
                y: y as f32 / block_region.height() as f32,
                width: width as f32 / block_region.width() as f32,
                height: height as f32 / block_region.height() as f32,
                confidence,
                description: Some("Detected text field element".to_string()),
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_to_block_coordinates_with_zero_grid_dimensions() {
        let parser = UniversalBlockParser::new();

        // Create a UBPResult with zero grid dimensions
        let ubp_result = UBPResult {
            image_width: 0,
            image_height: 0,
            grid_config: UBPConfig::default(),
            blocks: vec![],
            grid_dimensions: (0, 0), // Zero dimensions should cause error
            actual_block_size: 64,
            processing_time_ms: 0,
            total_elements_detected: 0,
        };

        let result = parser.global_to_block_coordinates(&ubp_result, 10.0, 10.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Grid dimensions cannot be zero"));
    }

    #[test]
    fn test_global_to_block_coordinates_coordinate_consistency() {
        let parser = UniversalBlockParser::new();

        // Create a simple UBPResult for testing
        let block = UBPBlock {
            id: 0,
            global_x: 0,
            global_y: 0,
            width: 64,
            height: 64,
            position_embedding: vec![0.0; 16],
            elements: vec![],
            density_score: 0.0,
        };

        let ubp_result = UBPResult {
            image_width: 64,
            image_height: 64,
            grid_config: UBPConfig::default(),
            blocks: vec![block],
            grid_dimensions: (1, 1),
            actual_block_size: 64,
            processing_time_ms: 0,
            total_elements_detected: 0,
        };

                // Test with out-of-bounds coordinates (should be clamped)
        let result = parser.global_to_block_coordinates(&ubp_result, 100.0, 100.0);
        assert!(result.is_ok());

        let coords = result.unwrap();
        // Should have lower confidence due to clamping
        assert_eq!(coords.confidence, 0.7);
        // Relative coordinates should be 63/64 = 0.984375 since coordinates are clamped to (63, 63)
        assert!((coords.relative_x - 0.984375).abs() < 0.001);
        assert!((coords.relative_y - 0.984375).abs() < 0.001);
    }

    #[test]
    fn test_global_to_block_coordinates_in_bounds() {
        let parser = UniversalBlockParser::new();

        let block = UBPBlock {
            id: 0,
            global_x: 0,
            global_y: 0,
            width: 64,
            height: 64,
            position_embedding: vec![0.0; 16],
            elements: vec![],
            density_score: 0.0,
        };

        let ubp_result = UBPResult {
            image_width: 64,
            image_height: 64,
            grid_config: UBPConfig::default(),
            blocks: vec![block],
            grid_dimensions: (1, 1),
            actual_block_size: 64,
            processing_time_ms: 0,
            total_elements_detected: 0,
        };

        // Test with in-bounds coordinates
        let result = parser.global_to_block_coordinates(&ubp_result, 32.0, 32.0);
        assert!(result.is_ok());

        let coords = result.unwrap();
        // Should have high confidence (no clamping)
        assert_eq!(coords.confidence, 0.95);
        // Should be in the middle of the block
        assert!((coords.relative_x - 0.5).abs() < 0.01);
        assert!((coords.relative_y - 0.5).abs() < 0.01);
    }
}
