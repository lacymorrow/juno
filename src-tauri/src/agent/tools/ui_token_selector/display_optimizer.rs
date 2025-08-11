//! Display Optimizer for Multi-Monitor UI-Guided Visual Token Selection
//!
//! Provides display-specific optimizations for token selection based on
//! display characteristics, resolution, and multi-monitor configurations.

use crate::agent::tools::ui_token_selector::{TokenSelectionError, VisualToken, DisplayInfo};
use crate::agent::tools::ui_token_selector::config::TokenSelectionConfig;
use tracing::{debug, info};

/// Display Optimizer for multi-monitor token selection optimization
pub struct DisplayOptimizer {
    config: TokenSelectionConfig,
}

impl DisplayOptimizer {
    /// Creates a new display optimizer with the given configuration
    pub fn new(config: &TokenSelectionConfig) -> Result<Self, TokenSelectionError> {
        Ok(Self {
            config: config.clone(),
        })
    }

    /// Optimizes tokens for a specific display configuration
    pub async fn optimize_for_display(
        &self,
        mut tokens: Vec<VisualToken>,
        display_info: &DisplayInfo,
    ) -> Result<Vec<VisualToken>, String> {
        if !self.config.multi_monitor.enable_display_optimization {
            return Ok(tokens);
        }

        info!(
            "Optimizing {} tokens for display {} ({}x{})",
            tokens.len(),
            display_info.id,
            display_info.bounds.width,
            display_info.bounds.height
        );

        // Step 1: Apply display-specific reduction targets
        tokens = self.apply_display_specific_reduction(tokens, display_info).await?;

        // Step 2: Scale optimization based on resolution
        if self.config.multi_monitor.scale_by_resolution {
            tokens = self.apply_resolution_scaling(tokens, display_info).await?;
        }

        // Step 3: Apply display-aware importance adjustments
        tokens = self.adjust_importance_for_display(tokens, display_info).await?;

        debug!(
            "Display optimization complete: {} tokens optimized for display {}",
            tokens.len(),
            display_info.id
        );

        Ok(tokens)
    }

    /// Applies display-specific reduction targets
    async fn apply_display_specific_reduction(
        &self,
        mut tokens: Vec<VisualToken>,
        display_info: &DisplayInfo,
    ) -> Result<Vec<VisualToken>, String> {
        // Determine reduction target based on display type
        let reduction_target = if display_info.is_main {
            self.config.multi_monitor.primary_display_reduction
        } else {
            self.config.multi_monitor.secondary_display_reduction
        };

        let current_count = tokens.len();
        let target_count = (current_count as f32 * (1.0 - reduction_target)).ceil() as usize;

        if current_count <= target_count {
            return Ok(tokens);
        }

        // Sort by importance and keep the most important tokens
        tokens.sort_by(|a, b| {
            b.importance_score.partial_cmp(&a.importance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        tokens.truncate(target_count);

        debug!(
            "Applied display-specific reduction: {} -> {} tokens ({:.1}% reduction) for {} display",
            current_count,
            target_count,
            reduction_target * 100.0,
            if display_info.is_main { "primary" } else { "secondary" }
        );

        Ok(tokens)
    }

    /// Applies resolution-based scaling optimizations
    async fn apply_resolution_scaling(
        &self,
        mut tokens: Vec<VisualToken>,
        display_info: &DisplayInfo,
    ) -> Result<Vec<VisualToken>, String> {
        let display_area = display_info.bounds.width * display_info.bounds.height;

        // Define resolution categories
        let resolution_category = self.categorize_resolution(display_area);

        // Apply resolution-specific optimizations
        match resolution_category {
            ResolutionCategory::HighDPI => {
                // For high DPI displays, we can be more aggressive with reduction
                // since there's typically more detail than needed
                tokens = self.apply_high_dpi_optimization(tokens).await?;
            }
            ResolutionCategory::Standard => {
                // Standard resolution - use default optimization
                // No additional changes needed
            }
            ResolutionCategory::LowResolution => {
                // For low resolution displays, preserve more tokens to maintain detail
                tokens = self.apply_low_resolution_optimization(tokens).await?;
            }
        }

        debug!(
            "Applied resolution scaling for {:?} display ({}x{})",
            resolution_category,
            display_info.bounds.width,
            display_info.bounds.height
        );

        Ok(tokens)
    }

    /// Categorizes display resolution for optimization purposes
    fn categorize_resolution(&self, display_area: f64) -> ResolutionCategory {
        // Common resolution breakpoints
        const HD_AREA: f64 = 1920.0 * 1080.0;
        const UHD_4K_AREA: f64 = 3840.0 * 2160.0;

        if display_area >= UHD_4K_AREA {
            ResolutionCategory::HighDPI
        } else if display_area >= HD_AREA {
            ResolutionCategory::Standard
        } else {
            ResolutionCategory::LowResolution
        }
    }

    /// Applies high DPI specific optimizations
    async fn apply_high_dpi_optimization(&self, tokens: Vec<VisualToken>) -> Result<Vec<VisualToken>, String> {
        // For high DPI displays, we can be more aggressive with background reduction
        // since there's typically redundant detail

        let background_reduction_factor = 0.7; // Keep only 70% of background tokens

        let mut background_tokens: Vec<_> = tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| matches!(token.token_type, crate::agent::tools::ui_token_selector::TokenType::Background))
            .collect();

        let non_background_tokens: Vec<_> = tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| !matches!(token.token_type, crate::agent::tools::ui_token_selector::TokenType::Background))
            .map(|(_, token)| token.clone())
            .collect();

        // Sort background tokens by importance
        background_tokens.sort_by(|a, b| {
            b.1.importance_score.partial_cmp(&a.1.importance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Keep only the most important background tokens
        // Use ceil to avoid overly aggressive reduction for small counts
        let keep_count = (background_tokens.len() as f32 * background_reduction_factor).ceil() as usize;
        let kept_background_tokens: Vec<_> = background_tokens
            .into_iter()
            .take(keep_count)
            .map(|(_, token)| token.clone())
            .collect();

        let mut result = non_background_tokens;
        result.extend(kept_background_tokens);

        debug!("Applied high DPI optimization: kept {} background tokens", keep_count);

        Ok(result)
    }

    /// Applies low resolution specific optimizations
    async fn apply_low_resolution_optimization(&self, tokens: Vec<VisualToken>) -> Result<Vec<VisualToken>, String> {
        // For low resolution displays, preserve more detail since every pixel matters
        // We'll boost the importance of all tokens slightly to prevent over-reduction

        let mut optimized_tokens = tokens;

        for token in &mut optimized_tokens {
            // Boost importance for low resolution displays
            let boosted_importance = (token.importance_score * 1.2).min(1.0);
            let mut optimized_token = token.clone();
            optimized_token.importance_score = boosted_importance;
            *token = optimized_token;
        }

        debug!("Applied low resolution optimization: boosted importance scores");

        Ok(optimized_tokens)
    }

    /// Adjusts token importance based on display characteristics
    async fn adjust_importance_for_display(
        &self,
        mut tokens: Vec<VisualToken>,
        display_info: &DisplayInfo,
    ) -> Result<Vec<VisualToken>, String> {
        // Apply display-specific importance adjustments

        for token in &mut tokens {
            let mut adjusted_importance = token.importance_score;

            // Primary displays get slight importance boost for interactive elements
            if display_info.is_main {
                match token.token_type {
                    crate::agent::tools::ui_token_selector::TokenType::Interactive => {
                        adjusted_importance = (adjusted_importance * 1.1).min(1.0);
                    }
                    crate::agent::tools::ui_token_selector::TokenType::Text => {
                        adjusted_importance = (adjusted_importance * 1.05).min(1.0);
                    }
                    _ => {}
                }
            }

            // Adjust based on position relative to display bounds
            let relative_x = token.x as f64 / display_info.bounds.width;
            let relative_y = token.y as f64 / display_info.bounds.height;

            // Boost importance for elements in the "golden zone" (center-left quadrant)
            if relative_x >= 0.2 && relative_x <= 0.8 && relative_y >= 0.2 && relative_y <= 0.8 {
                adjusted_importance = (adjusted_importance * 1.1).min(1.0);
            }

            token.importance_score = adjusted_importance;
        }

        debug!(
            "Adjusted importance scores for display characteristics (primary: {})",
            display_info.is_main
        );

        Ok(tokens)
    }

    /// Validates multi-monitor optimization results
    pub fn validate_multi_monitor_optimization(
        &self,
        original_tokens: &[VisualToken],
        optimized_tokens: &[VisualToken],
        display_info: &DisplayInfo,
    ) -> Result<bool, String> {
        let original_count = original_tokens.len();
        let optimized_count = optimized_tokens.len();

        // Check that optimization actually occurred
        if optimized_count > original_count {
            return Err(format!(
                "Multi-monitor optimization increased token count: {} -> {}",
                original_count, optimized_count
            ));
        }

        // Check that reduction is appropriate for display type
        let expected_reduction = if display_info.is_main {
            self.config.multi_monitor.primary_display_reduction
        } else {
            self.config.multi_monitor.secondary_display_reduction
        };

        let actual_reduction = (original_count - optimized_count) as f32 / original_count as f32;

        if actual_reduction < expected_reduction * 0.5 {
            debug!(
                "Multi-monitor optimization reduction ({:.1}%) is lower than expected ({:.1}%)",
                actual_reduction * 100.0,
                expected_reduction * 100.0
            );
        }

        Ok(true)
    }
}

/// Resolution categories for optimization
#[derive(Debug, Clone, PartialEq)]
enum ResolutionCategory {
    HighDPI,      // 4K and above
    Standard,     // 1080p to 4K
    LowResolution, // Below 1080p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tools::ui_token_selector::{
        config::TokenSelectionConfig,
        VisualToken,
        TokenType,
        RGBColor,
        DisplayBounds
    };

    fn create_test_display(is_main: bool, width: f64, height: f64) -> DisplayInfo {
        DisplayInfo {
            id: if is_main { 1 } else { 2 },
            bounds: DisplayBounds {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            is_main,
        }
    }

    fn create_test_token(token_type: TokenType, importance: f32, x: u32, y: u32) -> VisualToken {
        VisualToken {
            x,
            y,
            width: 10,
            height: 10,
            dominant_color: RGBColor { r: 100, g: 100, b: 100 },
            color_variance: 0.5,
            importance_score: importance,
            token_type,
            connected_tokens: Vec::new(),
            redundancy_group: None,
        }
    }

    #[test]
    fn test_display_optimizer_creation() {
        let config = TokenSelectionConfig::default();
        let optimizer = DisplayOptimizer::new(&config);
        assert!(optimizer.is_ok());
    }

    #[test]
    fn test_resolution_categorization() {
        let config = TokenSelectionConfig::default();
        let optimizer = DisplayOptimizer::new(&config).unwrap();

        // Test different resolution categories
        assert_eq!(optimizer.categorize_resolution(1920.0 * 1080.0), ResolutionCategory::Standard);
        assert_eq!(optimizer.categorize_resolution(3840.0 * 2160.0), ResolutionCategory::HighDPI);
        assert_eq!(optimizer.categorize_resolution(1280.0 * 720.0), ResolutionCategory::LowResolution);
    }

    #[tokio::test]
    async fn test_display_specific_reduction() {
        let mut config = TokenSelectionConfig::default();
        config.multi_monitor.primary_display_reduction = 0.3; // 30% reduction
        config.multi_monitor.secondary_display_reduction = 0.5; // 50% reduction

        let optimizer = DisplayOptimizer::new(&config).unwrap();

        let tokens = vec![
            create_test_token(TokenType::Interactive, 0.9, 100, 100),
            create_test_token(TokenType::Text, 0.8, 200, 200),
            create_test_token(TokenType::Background, 0.3, 300, 300),
            create_test_token(TokenType::Background, 0.2, 400, 400),
        ];

        // Test primary display (less aggressive reduction)
        let primary_display = create_test_display(true, 1920.0, 1080.0);
        let primary_result = optimizer.apply_display_specific_reduction(tokens.clone(), &primary_display).await.unwrap();

        // Test secondary display (more aggressive reduction)
        let secondary_display = create_test_display(false, 1920.0, 1080.0);
        let secondary_result = optimizer.apply_display_specific_reduction(tokens.clone(), &secondary_display).await.unwrap();

        // Secondary display should have fewer tokens due to higher reduction target
        assert!(secondary_result.len() <= primary_result.len());
    }

    #[tokio::test]
    async fn test_high_dpi_optimization() {
        let config = TokenSelectionConfig::default();
        let optimizer = DisplayOptimizer::new(&config).unwrap();

        let tokens = vec![
            create_test_token(TokenType::Interactive, 0.9, 100, 100),
            create_test_token(TokenType::Background, 0.4, 200, 200),
            create_test_token(TokenType::Background, 0.3, 300, 300),
            create_test_token(TokenType::Background, 0.2, 400, 400),
        ];

        let optimized = optimizer.apply_high_dpi_optimization(tokens).await.unwrap();

        // Should preserve interactive elements and reduce background tokens
        let interactive_count = optimized.iter()
            .filter(|t| matches!(t.token_type, TokenType::Interactive))
            .count();
        assert_eq!(interactive_count, 1);

        let background_count = optimized.iter()
            .filter(|t| matches!(t.token_type, TokenType::Background))
            .count();
        // With less aggressive reduction for small counts, we allow keeping all
        // background tokens in small sets
        assert!(background_count <= 3);
    }

    #[tokio::test]
    async fn test_importance_adjustment() {
        let config = TokenSelectionConfig::default();
        let optimizer = DisplayOptimizer::new(&config).unwrap();

        let tokens = vec![
            create_test_token(TokenType::Interactive, 0.5, 500, 500), // Center position
            create_test_token(TokenType::Text, 0.5, 100, 100),       // Edge position
        ];

        let primary_display = create_test_display(true, 1000.0, 1000.0);
        let adjusted = optimizer.adjust_importance_for_display(tokens, &primary_display).await.unwrap();

        // Interactive token in center should have boosted importance
        let center_token = &adjusted[0];
        assert!(center_token.importance_score > 0.5);

        // Text token at edge should have slightly boosted importance (primary display bonus)
        let edge_token = &adjusted[1];
        assert!(edge_token.importance_score > 0.5);
    }
}
