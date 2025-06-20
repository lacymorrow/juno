//! RGB Connected Graph Analyzer for UI-Guided Visual Token Selection
//!
//! Implements the ShowUI paper's approach to treating screenshots as connected graphs
//! using RGB color space analysis for identifying redundant visual relationships.

use crate::agent::tools::ui_token_selector::{TokenSelectionError, RGBColor, VisualToken, TokenType};
use crate::agent::tools::ui_token_selector::config::TokenSelectionConfig;
use image::{DynamicImage, ImageBuffer, Rgba};
use tracing::{debug, info, warn};

/// RGB Connected Graph representation for visual analysis
#[derive(Debug, Clone)]
pub struct RGBConnectedGraph {
    /// Image dimensions
    pub width: u32,
    pub height: u32,

    /// Visual tokens extracted from the image
    pub tokens: Vec<VisualToken>,

    /// Connection matrix between tokens
    pub connections: Vec<Vec<bool>>,

    /// Redundancy groups
    pub redundancy_groups: Vec<Vec<u32>>,
}

/// Image patch for advanced analysis
#[derive(Debug, Clone)]
pub struct ImagePatch {
    pub id: usize,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub average_color: RGBAColor,
    pub patch_type: PatchType,
    pub importance_score: f32,
}

/// RGBA Color representation
#[derive(Debug, Clone)]
pub struct RGBAColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Connected component for graph analysis
#[derive(Debug, Clone)]
pub struct ConnectedComponent {
    pub id: usize,
    pub patch_indices: Vec<usize>,
    pub average_similarity: f32,
    pub total_area: u32,
}

/// RGB analysis result with comprehensive metrics
#[derive(Debug)]
pub struct RGBAnalysisResult {
    pub patches: Vec<ImagePatch>,
    pub connected_components: Vec<ConnectedComponent>,
    pub importance_scores: Vec<f32>,
    pub processing_time_ms: u64,
    pub patch_size: PatchSize,
    pub similarity_threshold: f32,
    pub total_patches: usize,
}

/// RGB Connected Graph Analyzer
pub struct RGBConnectedGraphAnalyzer {
    config: TokenSelectionConfig,
}

impl RGBConnectedGraphAnalyzer {
    /// Creates a new RGB analyzer with the given configuration
    pub fn new(config: &TokenSelectionConfig) -> Result<Self, TokenSelectionError> {
        Ok(Self {
            config: config.clone(),
        })
    }

    /// Analyzes an image and creates an RGB connected graph
    pub async fn analyze_image(&self, image: &DynamicImage) -> Result<RGBConnectedGraph, String> {
        let (width, height) = (image.width(), image.height());

        info!("Analyzing {}x{} image with RGB connected graph approach", width, height);

        // Convert to RGB format for analysis
        let rgb_image = image.to_rgb8();

        // Step 1: Extract visual tokens using patch-based analysis
        let tokens = self.extract_visual_tokens(&rgb_image).await?;
        debug!("Extracted {} visual tokens", tokens.len());

        // Step 2: Build connection matrix based on color similarity
        let connections = self.build_connection_matrix(&tokens).await?;
        debug!("Built connection matrix with {} connections",
               connections.iter().flatten().filter(|&&x| x).count());

        // Step 3: Identify redundancy groups
        let redundancy_groups = self.identify_redundancy_groups(&tokens, &connections).await?;
        debug!("Identified {} redundancy groups", redundancy_groups.len());

        Ok(RGBConnectedGraph {
            width,
            height,
            tokens,
            connections,
            redundancy_groups,
        })
    }

    /// Extracts visual tokens from an RGB image using patch-based analysis
    async fn extract_visual_tokens(&self, image: &image::RgbImage) -> Result<Vec<VisualToken>, String> {
        let (width, height) = image.dimensions();
        let mut tokens = Vec::new();
        let mut token_id = 0;

        let patch_size = self.config.rgb_analysis.min_patch_size;

        // Scan image in patches
        for y in (0..height).step_by(patch_size as usize) {
            for x in (0..width).step_by(patch_size as usize) {
                let patch_width = (patch_size).min(width - x);
                let patch_height = (patch_size).min(height - y);

                // Analyze patch
                let (dominant_color, color_variance) = self.analyze_patch(image, x, y, patch_width, patch_height)?;

                // Calculate importance score based on color variance and position
                let importance_score = self.calculate_importance_score(
                    &dominant_color,
                    color_variance,
                    x, y,
                    width, height
                );

                // Classify token type
                let token_type = self.classify_token_type(&dominant_color, color_variance, importance_score);

                let token = VisualToken {
                    x,
                    y,
                    width: patch_width,
                    height: patch_height,
                    dominant_color,
                    color_variance,
                    importance_score,
                    token_type,
                    connected_tokens: Vec::new(), // Will be populated later
                    redundancy_group: None,       // Will be populated later
                };

                tokens.push(token);
                token_id += 1;
            }
        }

        Ok(tokens)
    }

    /// Analyzes a patch to extract dominant color and variance
    fn analyze_patch(
        &self,
        image: &image::RgbImage,
        x: u32,
        y: u32,
        width: u32,
        height: u32
    ) -> Result<(RGBColor, f32), String> {
        let mut r_sum = 0u64;
        let mut g_sum = 0u64;
        let mut b_sum = 0u64;
        let mut pixel_count = 0u64;

        // Calculate average color
        for py in y..y + height {
            for px in x..x + width {
                if let Some(pixel) = image.get_pixel_checked(px, py) {
                    r_sum += pixel[0] as u64;
                    g_sum += pixel[1] as u64;
                    b_sum += pixel[2] as u64;
                    pixel_count += 1;
                }
            }
        }

        if pixel_count == 0 {
            return Ok((RGBColor { r: 0, g: 0, b: 0 }, 0.0));
        }

        let avg_r = (r_sum / pixel_count) as u8;
        let avg_g = (g_sum / pixel_count) as u8;
        let avg_b = (b_sum / pixel_count) as u8;

        let dominant_color = RGBColor {
            r: avg_r,
            g: avg_g,
            b: avg_b,
        };

        // Calculate color variance
        let mut variance_sum = 0.0;
        for py in y..y + height {
            for px in x..x + width {
                if let Some(pixel) = image.get_pixel_checked(px, py) {
                    let r_diff = pixel[0] as f32 - avg_r as f32;
                    let g_diff = pixel[1] as f32 - avg_g as f32;
                    let b_diff = pixel[2] as f32 - avg_b as f32;
                    variance_sum += r_diff * r_diff + g_diff * g_diff + b_diff * b_diff;
                }
            }
        }

        let color_variance = if pixel_count > 0 {
            (variance_sum / pixel_count as f32).sqrt() / 255.0 // Normalize to 0-1
        } else {
            0.0
        };

        Ok((dominant_color, color_variance))
    }

    /// Calculate patch average color
    fn calculate_patch_average_color(
        &self,
        image_buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<RGBAColor, TokenSelectorError> {
        let mut r_sum = 0u64;
        let mut g_sum = 0u64;
        let mut b_sum = 0u64;
        let mut a_sum = 0u64;
        let mut pixel_count = 0u64;

        for py in y..(y + height).min(image_buffer.height()) {
            for px in x..(x + width).min(image_buffer.width()) {
                let pixel = image_buffer.get_pixel(px, py);
                r_sum += pixel[0] as u64;
                g_sum += pixel[1] as u64;
                b_sum += pixel[2] as u64;
                a_sum += pixel[3] as u64;
                pixel_count += 1;
            }
        }

        if pixel_count == 0 {
            return Ok(RGBAColor { r: 0, g: 0, b: 0, a: 255 });
        }

        Ok(RGBAColor {
            r: (r_sum / pixel_count) as u8,
            g: (g_sum / pixel_count) as u8,
            b: (b_sum / pixel_count) as u8,
            a: (a_sum / pixel_count) as u8,
        })
    }

    /// Calculate color distance between two RGBA colors
    fn calculate_color_distance(&self, color1: &RGBAColor, color2: &RGBAColor) -> f32 {
        let r_diff = color1.r as f32 - color2.r as f32;
        let g_diff = color1.g as f32 - color2.g as f32;
        let b_diff = color1.b as f32 - color2.b as f32;

        (r_diff * r_diff + g_diff * g_diff + b_diff * b_diff).sqrt()
    }

    /// Calculate pixel distance for edge detection
    fn calculate_pixel_distance(&self, pixel1: &Rgba<u8>, pixel2: &Rgba<u8>) -> f32 {
        let r_diff = pixel1[0] as f32 - pixel2[0] as f32;
        let g_diff = pixel1[1] as f32 - pixel2[1] as f32;
        let b_diff = pixel1[2] as f32 - pixel2[2] as f32;

        (r_diff * r_diff + g_diff * g_diff + b_diff * b_diff).sqrt()
    }

    /// Calculates importance score for a token
    fn calculate_importance_score(
        &self,
        color: &RGBColor,
        variance: f32,
        x: u32,
        y: u32,
        image_width: u32,
        image_height: u32,
    ) -> f32 {
        let mut score = 0.0;

        // Higher variance = more important (likely contains details)
        score += variance * 0.4;

        // Edge pixels are more important
        let edge_factor = if x == 0 || y == 0 || x >= image_width - 32 || y >= image_height - 32 {
            0.2
        } else {
            0.0
        };
        score += edge_factor;

        // Bright colors are often more important (UI elements)
        let brightness = (color.r as f32 + color.g as f32 + color.b as f32) / (3.0 * 255.0);
        score += brightness * 0.2;

        // Center-weighted importance (UI elements often in center)
        let center_x = image_width as f32 / 2.0;
        let center_y = image_height as f32 / 2.0;
        let distance_from_center = ((x as f32 - center_x).powi(2) + (y as f32 - center_y).powi(2)).sqrt();
        let max_distance = (center_x.powi(2) + center_y.powi(2)).sqrt();
        let center_factor = 1.0 - (distance_from_center / max_distance);
        score += center_factor * 0.2;

        score.min(1.0) // Clamp to 0-1 range
    }

    /// Classifies token type based on visual characteristics
    fn classify_token_type(&self, color: &RGBColor, variance: f32, importance: f32) -> TokenType {
        // High variance usually indicates text or interactive elements
        if variance > 0.3 {
            if importance > 0.6 {
                TokenType::Interactive
            } else {
                TokenType::Text
            }
        } else if variance < 0.1 {
            // Low variance indicates solid colors (background or chrome)
            let brightness = (color.r as f32 + color.g as f32 + color.b as f32) / (3.0 * 255.0);
            if brightness < 0.2 || brightness > 0.8 {
                TokenType::Background
            } else {
                TokenType::Chrome
            }
        } else {
            TokenType::Unknown
        }
    }

    /// Builds connection matrix between tokens based on color similarity
    async fn build_connection_matrix(&self, tokens: &[VisualToken]) -> Result<Vec<Vec<bool>>, String> {
        let token_count = tokens.len();
        let mut connections = vec![vec![false; token_count]; token_count];

        let similarity_threshold = self.config.rgb_analysis.color_similarity_threshold;

        for i in 0..token_count {
            for j in i + 1..token_count {
                if self.are_colors_similar(&tokens[i].dominant_color, &tokens[j].dominant_color, similarity_threshold) {
                    connections[i][j] = true;
                    connections[j][i] = true;
                }
            }
        }

        Ok(connections)
    }

    /// Checks if two colors are similar within the threshold
    fn are_colors_similar(&self, color1: &RGBColor, color2: &RGBColor, threshold: f32) -> bool {
        let r_diff = (color1.r as f32 - color2.r as f32) / 255.0;
        let g_diff = (color1.g as f32 - color2.g as f32) / 255.0;
        let b_diff = (color1.b as f32 - color2.b as f32) / 255.0;

        let distance = (r_diff * r_diff + g_diff * g_diff + b_diff * b_diff).sqrt();
        distance <= threshold
    }

    /// Identifies redundancy groups using connected components
    async fn identify_redundancy_groups(&self, tokens: &[VisualToken], connections: &[Vec<bool>]) -> Result<Vec<Vec<u32>>, String> {
        let token_count = tokens.len();
        let mut visited = vec![false; token_count];
        let mut groups = Vec::new();

        for i in 0..token_count {
            if !visited[i] {
                let mut group = Vec::new();
                self.dfs_collect_group(i, &mut visited, &mut group, connections);

                // Only consider it a redundancy group if it has multiple tokens
                if group.len() > 1 {
                    groups.push(group);
                }
            }
        }

        Ok(groups)
    }

    /// Depth-first search to collect connected tokens into a group
    fn dfs_collect_group(&self, token_idx: usize, visited: &mut [bool], group: &mut Vec<u32>, connections: &[Vec<bool>]) {
        visited[token_idx] = true;
        group.push(token_idx as u32);

        for j in 0..connections[token_idx].len() {
            if connections[token_idx][j] && !visited[j] {
                self.dfs_collect_group(j, visited, group, connections);
            }
        }
    }

    /// Advanced RGB analysis with adaptive patch sizing based on image characteristics
    /// Week 2 Enhancement: Adaptive patch sizing for better UI element detection
    pub async fn analyze_with_adaptive_patches(
        &self,
        image_buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        display_info: &DisplayInfo,
    ) -> Result<RGBAnalysisResult, TokenSelectorError> {
        let start_time = std::time::Instant::now();

        // Determine optimal patch size based on image characteristics
        let adaptive_patch_size = self.calculate_adaptive_patch_size(image_buffer, display_info)?;

        info!(
            "Using adaptive patch size: {}x{} for {}x{} image on display {}",
            adaptive_patch_size.width,
            adaptive_patch_size.height,
            image_buffer.width(),
            image_buffer.height(),
            display_info.id
        );

        // Create patches with adaptive sizing
        let patches = self.create_adaptive_patches(image_buffer, adaptive_patch_size)?;

        // Perform advanced color similarity analysis
        let similarity_graph = self.build_advanced_similarity_graph(&patches, display_info).await?;

        // Find connected components with importance weighting
        let connected_components = self.find_weighted_connected_components(&similarity_graph, &patches)?;

        // Calculate importance scores with UI-specific heuristics
        let importance_scores = self.calculate_ui_importance_scores(&patches, &connected_components, display_info)?;

        let processing_time = start_time.elapsed();

        Ok(RGBAnalysisResult {
            patches,
            connected_components,
            importance_scores,
            processing_time_ms: processing_time.as_millis() as u64,
            patch_size: adaptive_patch_size,
            similarity_threshold: self.config.rgb_analysis.color_similarity_threshold,
            total_patches: patches.len(),
        })
    }

    /// Calculate optimal patch size based on image characteristics and display type
    fn calculate_adaptive_patch_size(
        &self,
        image_buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        display_info: &DisplayInfo,
    ) -> Result<PatchSize, TokenSelectorError> {
        let width = image_buffer.width();
        let height = image_buffer.height();

        // Base patch size calculation
        let base_size = match display_info.resolution_category {
            ResolutionCategory::HighDPI => 32,      // 4K+ displays
            ResolutionCategory::Standard => 24,     // 1080p-1440p displays
            ResolutionCategory::Low => 16,          // Sub-1080p displays
        };

        // Adaptive sizing based on image complexity
        let complexity_factor = self.calculate_image_complexity(image_buffer)?;
        let adaptive_size = (base_size as f32 * complexity_factor).round() as u32;

        // Ensure minimum and maximum patch sizes
        let final_size = adaptive_size.clamp(8, 64);

        // Adjust for aspect ratio if needed
        let aspect_ratio = width as f32 / height as f32;
        let (patch_width, patch_height) = if aspect_ratio > 1.5 {
            // Wide display - use wider patches
            (final_size + 4, final_size)
        } else if aspect_ratio < 0.7 {
            // Tall display - use taller patches
            (final_size, final_size + 4)
        } else {
            // Standard aspect ratio
            (final_size, final_size)
        };

        debug!(
            "Adaptive patch sizing: base={}, complexity_factor={:.2}, final={}x{} for {}x{} image",
            base_size, complexity_factor, patch_width, patch_height, width, height
        );

        Ok(PatchSize {
            width: patch_width,
            height: patch_height,
        })
    }

    /// Calculate image complexity factor for adaptive patch sizing
    fn calculate_image_complexity(
        &self,
        image_buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    ) -> Result<f32, TokenSelectorError> {
        let width = image_buffer.width();
        let height = image_buffer.height();

        // Sample points for complexity analysis (every 8th pixel)
        let mut color_variations = 0;
        let mut total_samples = 0;
        let mut edge_count = 0;

        for y in (0..height).step_by(8) {
            for x in (0..width).step_by(8) {
                if x + 8 < width && y + 8 < height {
                    let current_pixel = image_buffer.get_pixel(x, y);
                    let right_pixel = image_buffer.get_pixel(x + 8, y);
                    let down_pixel = image_buffer.get_pixel(x, y + 8);

                    // Calculate color variation
                    let right_diff = self.calculate_pixel_distance(current_pixel, right_pixel);
                    let down_diff = self.calculate_pixel_distance(current_pixel, down_pixel);

                    if right_diff > 30.0 || down_diff > 30.0 {
                        color_variations += 1;
                    }

                    // Edge detection (simple gradient)
                    if right_diff > 50.0 || down_diff > 50.0 {
                        edge_count += 1;
                    }

                    total_samples += 1;
                }
            }
        }

        if total_samples == 0 {
            return Ok(1.0);
        }

        // Calculate complexity metrics
        let variation_ratio = color_variations as f32 / total_samples as f32;
        let edge_ratio = edge_count as f32 / total_samples as f32;

        // Combine metrics into complexity factor (0.5 to 1.5 range)
        let complexity_factor = 0.5 + (variation_ratio * 0.7) + (edge_ratio * 0.3);

        debug!(
            "Image complexity analysis: variations={:.2}%, edges={:.2}%, factor={:.2}",
            variation_ratio * 100.0, edge_ratio * 100.0, complexity_factor
        );

        Ok(complexity_factor.clamp(0.5, 1.5))
    }

    /// Create patches with adaptive sizing
    fn create_adaptive_patches(
        &self,
        image_buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        patch_size: PatchSize,
    ) -> Result<Vec<ImagePatch>, TokenSelectorError> {
        let width = image_buffer.width();
        let height = image_buffer.height();
        let mut patches = Vec::new();

        // Calculate overlap for better coverage
        let overlap_x = patch_size.width / 4;
        let overlap_y = patch_size.height / 4;
        let step_x = patch_size.width - overlap_x;
        let step_y = patch_size.height - overlap_y;

        for y in (0..height).step_by(step_y as usize) {
            for x in (0..width).step_by(step_x as usize) {
                let patch_width = (patch_size.width).min(width - x);
                let patch_height = (patch_size.height).min(height - y);

                if patch_width >= 8 && patch_height >= 8 {
                    // Calculate average color for this patch
                    let avg_color = self.calculate_patch_average_color(
                        image_buffer, x, y, patch_width, patch_height
                    )?;

                    // Determine patch type based on color characteristics
                    let patch_type = self.classify_patch_type(&avg_color, image_buffer, x, y, patch_width, patch_height)?;

                    patches.push(ImagePatch {
                        id: patches.len(),
                        x,
                        y,
                        width: patch_width,
                        height: patch_height,
                        average_color: avg_color,
                        patch_type,
                        importance_score: 0.0, // Will be calculated later
                    });
                }
            }
        }

        info!("Created {} adaptive patches with {}x{} base size", patches.len(), patch_size.width, patch_size.height);
        Ok(patches)
    }

    /// Classify patch type based on visual characteristics
    fn classify_patch_type(
        &self,
        avg_color: &RGBAColor,
        image_buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<PatchType, TokenSelectorError> {
        // Analyze color uniformity
        let mut color_variance = 0.0;
        let mut pixel_count = 0;

        for py in y..(y + height).min(image_buffer.height()) {
            for px in x..(x + width).min(image_buffer.width()) {
                let pixel = image_buffer.get_pixel(px, py);
                let pixel_color = RGBAColor {
                    r: pixel[0],
                    g: pixel[1],
                    b: pixel[2],
                    a: pixel[3],
                };

                let distance = self.calculate_color_distance(avg_color, &pixel_color);
                color_variance += distance * distance;
                pixel_count += 1;
            }
        }

        if pixel_count == 0 {
            return Ok(PatchType::Background);
        }

        color_variance /= pixel_count as f32;
        let color_std_dev = color_variance.sqrt();

        // Classification based on color characteristics
        if color_std_dev < 10.0 {
            // Very uniform color - likely background
            if avg_color.r > 240 && avg_color.g > 240 && avg_color.b > 240 {
                PatchType::Background
            } else if avg_color.r < 50 && avg_color.g < 50 && avg_color.b < 50 {
                PatchType::Text
            } else {
                PatchType::Background
            }
        } else if color_std_dev < 30.0 {
            // Moderate variance - could be UI element
            PatchType::UIElement
        } else {
            // High variance - likely interactive element or content
            if self.has_high_contrast_edges(image_buffer, x, y, width, height)? {
                PatchType::Interactive
            } else {
                PatchType::Content
            }
        }
    }

    /// Check for high contrast edges indicating interactive elements
    fn has_high_contrast_edges(
        &self,
        image_buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<bool, TokenSelectorError> {
        let mut edge_count = 0;
        let mut total_checks = 0;

        // Check horizontal edges
        for py in y..(y + height - 1).min(image_buffer.height() - 1) {
            for px in x..(x + width - 1).min(image_buffer.width() - 1) {
                let current = image_buffer.get_pixel(px, py);
                let right = image_buffer.get_pixel(px + 1, py);
                let down = image_buffer.get_pixel(px, py + 1);

                let right_diff = self.calculate_pixel_distance(current, right);
                let down_diff = self.calculate_pixel_distance(current, down);

                if right_diff > 80.0 || down_diff > 80.0 {
                    edge_count += 1;
                }
                total_checks += 1;
            }
        }

        let edge_ratio = if total_checks > 0 {
            edge_count as f32 / total_checks as f32
        } else {
            0.0
        };

        Ok(edge_ratio > 0.15) // 15% edge threshold
    }

    /// Build advanced similarity graph with display-aware weighting
    async fn build_advanced_similarity_graph(
        &self,
        patches: &[ImagePatch],
        display_info: &DisplayInfo,
    ) -> Result<SimilarityGraph, TokenSelectorError> {
        let mut graph = SimilarityGraph::new(patches.len());

        // Calculate similarities in parallel chunks
        let chunk_size = 1000;
        let mut similarity_tasks = Vec::new();

        for chunk_start in (0..patches.len()).step_by(chunk_size) {
            let chunk_end = (chunk_start + chunk_size).min(patches.len());
            let chunk_patches = &patches[chunk_start..chunk_end];
            let all_patches = patches.to_vec();
            let display_info_clone = display_info.clone();
            let config = self.config.clone();

            let task = tokio::spawn(async move {
                let mut chunk_similarities = Vec::new();

                for (i, patch1) in chunk_patches.iter().enumerate() {
                    let patch1_idx = chunk_start + i;

                    for (j, patch2) in all_patches.iter().enumerate() {
                        if patch1_idx >= j {
                            continue; // Skip self and already processed pairs
                        }

                        // Calculate base color similarity
                        let color_similarity = 1.0 - Self::calculate_color_distance_static(
                            &patch1.average_color,
                            &patch2.average_color,
                        ) / 255.0;

                        if color_similarity < config.rgb_analysis.color_similarity_threshold {
                            continue;
                        }

                        // Apply spatial proximity weighting
                        let spatial_weight = Self::calculate_spatial_weight_static(
                            patch1, patch2, &display_info_clone
                        );

                        // Apply patch type compatibility
                        let type_weight = Self::calculate_type_compatibility_static(
                            &patch1.patch_type, &patch2.patch_type
                        );

                        // Combined similarity score
                        let final_similarity = color_similarity * spatial_weight * type_weight;

                        if final_similarity >= config.rgb_analysis.color_similarity_threshold {
                            chunk_similarities.push((patch1_idx, j, final_similarity));
                        }
                    }
                }

                chunk_similarities
            });

            similarity_tasks.push(task);
        }

        // Collect results from all tasks
        for task in similarity_tasks {
            let chunk_similarities = task.await
                .map_err(|e| TokenSelectionError::ProcessingError(format!("Similarity calculation failed: {}", e)))?;

            for (i, j, similarity) in chunk_similarities {
                graph.add_edge(i, j, similarity);
            }
        }

        info!("Built advanced similarity graph with {} edges", graph.edge_count());
        Ok(graph)
    }

    /// Calculate spatial proximity weight for patch similarity
    fn calculate_spatial_weight_static(
        patch1: &ImagePatch,
        patch2: &ImagePatch,
        display_info: &DisplayInfo,
    ) -> f32 {
        let center1_x = patch1.x as f32 + patch1.width as f32 / 2.0;
        let center1_y = patch1.y as f32 + patch1.height as f32 / 2.0;
        let center2_x = patch2.x as f32 + patch2.width as f32 / 2.0;
        let center2_y = patch2.y as f32 + patch2.height as f32 / 2.0;

        let distance = ((center1_x - center2_x).powi(2) + (center1_y - center2_y).powi(2)).sqrt();

        // Normalize distance by display size
        let max_distance = ((display_info.width.pow(2) + display_info.height.pow(2)) as f32).sqrt();
        let normalized_distance = distance / max_distance;

        // Exponential decay for spatial weight
        let spatial_weight = (-normalized_distance * 3.0).exp();

        // Boost weight for very close patches
        if distance < 100.0 {
            (spatial_weight * 1.5).min(1.0)
        } else {
            spatial_weight
        }
    }

    /// Calculate patch type compatibility weight
    fn calculate_type_compatibility_static(type1: &PatchType, type2: &PatchType) -> f32 {
        match (type1, type2) {
            // Same types have high compatibility
            (PatchType::Background, PatchType::Background) => 1.0,
            (PatchType::Text, PatchType::Text) => 0.9,
            (PatchType::UIElement, PatchType::UIElement) => 0.8,
            (PatchType::Interactive, PatchType::Interactive) => 0.7,
            (PatchType::Content, PatchType::Content) => 0.8,

            // Background can merge with most things
            (PatchType::Background, _) | (_, PatchType::Background) => 0.9,

            // UI elements can merge with content
            (PatchType::UIElement, PatchType::Content) | (PatchType::Content, PatchType::UIElement) => 0.6,

            // Interactive elements should generally stay separate
            (PatchType::Interactive, _) | (_, PatchType::Interactive) => 0.3,

            // Text should generally stay separate unless with background
            (PatchType::Text, _) | (_, PatchType::Text) => 0.4,
        }
    }

    /// Find connected components with importance weighting
    fn find_weighted_connected_components(
        &self,
        graph: &SimilarityGraph,
        patches: &[ImagePatch],
    ) -> Result<Vec<ConnectedComponent>, TokenSelectorError> {
        let mut visited = vec![false; patches.len()];
        let mut components = Vec::new();

        for start_idx in 0..patches.len() {
            if visited[start_idx] {
                continue;
            }

            // Perform weighted DFS to find component
            let mut component_patches = Vec::new();
            let mut stack = vec![start_idx];
            let mut total_weight = 0.0;

            while let Some(current_idx) = stack.pop() {
                if visited[current_idx] {
                    continue;
                }

                visited[current_idx] = true;
                component_patches.push(current_idx);

                // Add connected neighbors
                for (neighbor_idx, weight) in graph.get_neighbors(current_idx) {
                    if !visited[*neighbor_idx] && *weight >= self.config.rgb_analysis.color_similarity_threshold {
                        stack.push(*neighbor_idx);
                        total_weight += *weight;
                    }
                }
            }

            if !component_patches.is_empty() {
                let avg_weight = if component_patches.len() > 1 {
                    total_weight / (component_patches.len() - 1) as f32
                } else {
                    1.0
                };

                components.push(ConnectedComponent {
                    id: components.len(),
                    patch_indices: component_patches,
                    average_similarity: avg_weight,
                    total_area: 0, // Will be calculated later
                });
            }
        }

        info!("Found {} weighted connected components", components.len());
        Ok(components)
    }

    /// Calculate UI-specific importance scores
    fn calculate_ui_importance_scores(
        &self,
        patches: &[ImagePatch],
        components: &[ConnectedComponent],
        display_info: &DisplayInfo,
    ) -> Result<Vec<f32>, TokenSelectorError> {
        let mut importance_scores = vec![0.0; patches.len()];

        for component in components {
            let component_importance = self.calculate_component_importance(
                component, patches, display_info
            )?;

            // Distribute importance to all patches in component
            for &patch_idx in &component.patch_indices {
                importance_scores[patch_idx] = component_importance;
            }
        }

        // Normalize scores to 0-1 range
        let max_score = importance_scores.iter().fold(0.0f32, |acc, &score| acc.max(score));
        if max_score > 0.0 {
            for score in &mut importance_scores {
                *score /= max_score;
            }
        }

        debug!("Calculated UI importance scores for {} patches", patches.len());
        Ok(importance_scores)
    }

    /// Calculate importance score for a connected component
    fn calculate_component_importance(
        &self,
        component: &ConnectedComponent,
        patches: &[ImagePatch],
        display_info: &DisplayInfo,
    ) -> Result<f32, TokenSelectorError> {
        let mut total_importance = 0.0;
        let mut total_area = 0;

        for &patch_idx in &component.patch_indices {
            let patch = &patches[patch_idx];
            let patch_area = patch.width * patch.height;
            total_area += patch_area;

            // Base importance by patch type
            let type_importance = match patch.patch_type {
                PatchType::Interactive => 1.0,
                PatchType::Text => 0.8,
                PatchType::UIElement => 0.6,
                PatchType::Content => 0.4,
                PatchType::Background => 0.1,
            };

            // Position-based importance (center of screen is more important)
            let center_x = display_info.width as f32 / 2.0;
            let center_y = display_info.height as f32 / 2.0;
            let patch_center_x = patch.x as f32 + patch.width as f32 / 2.0;
            let patch_center_y = patch.y as f32 + patch.height as f32 / 2.0;

            let distance_from_center = ((patch_center_x - center_x).powi(2) +
                                      (patch_center_y - center_y).powi(2)).sqrt();
            let max_distance = ((center_x).powi(2) + (center_y).powi(2)).sqrt();
            let position_importance = 1.0 - (distance_from_center / max_distance).min(1.0);

            // Size-based importance (moderate size is more important than very large or very small)
            let patch_size_ratio = patch_area as f32 / (display_info.width * display_info.height) as f32;
            let size_importance = if patch_size_ratio < 0.001 {
                0.3 // Very small patches
            } else if patch_size_ratio > 0.5 {
                0.2 // Very large patches (likely background)
            } else {
                1.0 // Moderate size patches
            };

            let patch_importance = type_importance * position_importance * size_importance;
            total_importance += patch_importance * patch_area as f32;
        }

        let component_importance = if total_area > 0 {
            total_importance / total_area as f32
        } else {
            0.0
        };

        Ok(component_importance)
    }

    // Helper method for static color distance calculation
    fn calculate_color_distance_static(color1: &RGBAColor, color2: &RGBAColor) -> f32 {
        let r_diff = color1.r as f32 - color2.r as f32;
        let g_diff = color1.g as f32 - color2.g as f32;
        let b_diff = color1.b as f32 - color2.b as f32;

        (r_diff * r_diff + g_diff * g_diff + b_diff * b_diff).sqrt()
    }
}

// New types for Week 2 enhancements
#[derive(Debug, Clone)]
pub struct PatchSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatchType {
    Background,
    Text,
    UIElement,
    Interactive,
    Content,
}

#[derive(Debug, Clone)]
pub enum ResolutionCategory {
    HighDPI,    // 4K+ displays
    Standard,   // 1080p-1440p displays
    Low,        // Sub-1080p displays
}

#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub resolution_category: ResolutionCategory,
    pub is_primary: bool,
}

impl DisplayInfo {
    pub fn new(id: u32, width: u32, height: u32, is_primary: bool) -> Self {
        let resolution_category = if width >= 3840 || height >= 2160 {
            ResolutionCategory::HighDPI
        } else if width >= 1920 || height >= 1080 {
            ResolutionCategory::Standard
        } else {
            ResolutionCategory::Low
        };

        Self {
            id,
            width,
            height,
            resolution_category,
            is_primary,
        }
    }
}

#[derive(Debug)]
pub struct SimilarityGraph {
    adjacency_list: Vec<Vec<(usize, f32)>>,
    edge_count: usize,
}

impl SimilarityGraph {
    pub fn new(node_count: usize) -> Self {
        Self {
            adjacency_list: vec![Vec::new(); node_count],
            edge_count: 0,
        }
    }

    pub fn add_edge(&mut self, from: usize, to: usize, weight: f32) {
        if from < self.adjacency_list.len() && to < self.adjacency_list.len() {
            self.adjacency_list[from].push((to, weight));
            self.adjacency_list[to].push((from, weight));
            self.edge_count += 1;
        }
    }

    pub fn get_neighbors(&self, node: usize) -> &[(usize, f32)] {
        if node < self.adjacency_list.len() {
            &self.adjacency_list[node]
        } else {
            &[]
        }
    }

    pub fn edge_count(&self) -> usize {
        self.edge_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tools::ui_token_selector::config::TokenSelectionConfig;
    use image::{RgbImage, Rgb};

    #[test]
    fn test_rgb_analyzer_creation() {
        let config = TokenSelectionConfig::default();
        let analyzer = RGBConnectedGraphAnalyzer::new(&config);
        assert!(analyzer.is_ok());
    }

    #[test]
    fn test_color_similarity() {
        let config = TokenSelectionConfig::default();
        let analyzer = RGBConnectedGraphAnalyzer::new(&config).unwrap();

        let color1 = RGBColor { r: 100, g: 100, b: 100 };
        let color2 = RGBColor { r: 105, g: 105, b: 105 };
        let color3 = RGBColor { r: 200, g: 200, b: 200 };

        assert!(analyzer.are_colors_similar(&color1, &color2, 0.2));
        assert!(!analyzer.are_colors_similar(&color1, &color3, 0.2));
    }

    #[test]
    fn test_patch_analysis() {
        let config = TokenSelectionConfig::default();
        let analyzer = RGBConnectedGraphAnalyzer::new(&config).unwrap();

        // Create a simple 4x4 red image
        let mut image = RgbImage::new(4, 4);
        for pixel in image.pixels_mut() {
            *pixel = Rgb([255, 0, 0]);
        }

        let result = analyzer.analyze_patch(&image, 0, 0, 4, 4);
        assert!(result.is_ok());

        let (color, variance) = result.unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 0);
        assert_eq!(variance, 0.0); // Should be 0 for solid color
    }

    #[test]
    fn test_importance_calculation() {
        let config = TokenSelectionConfig::default();
        let analyzer = RGBConnectedGraphAnalyzer::new(&config).unwrap();

        let color = RGBColor { r: 255, g: 255, b: 255 }; // Bright white
        let score = analyzer.calculate_importance_score(&color, 0.5, 50, 50, 100, 100);

        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_token_classification() {
        let config = TokenSelectionConfig::default();
        let analyzer = RGBConnectedGraphAnalyzer::new(&config).unwrap();

        // High variance, high importance -> Interactive
        let token_type = analyzer.classify_token_type(&RGBColor { r: 100, g: 100, b: 100 }, 0.5, 0.8);
        assert!(matches!(token_type, TokenType::Interactive));

        // Low variance, low brightness -> Background
        let token_type = analyzer.classify_token_type(&RGBColor { r: 10, g: 10, b: 10 }, 0.05, 0.3);
        assert!(matches!(token_type, TokenType::Background));
    }
}
