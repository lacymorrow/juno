//! Token Reducer for UI-Guided Visual Token Selection
//!
//! Implements token reduction algorithms based on the ShowUI paper's approach
//! to achieve 33% computational cost reduction while preserving critical UI elements.

use crate::agent::tools::ui_token_selector::config::TokenSelectionConfig;
use crate::agent::tools::ui_token_selector::rgb_analyzer::RGBConnectedGraph;
use crate::agent::tools::ui_token_selector::{
    TokenReductionStats, TokenSelectionError, TokenType, VisualToken,
};
use tracing::{debug, info, warn};

/// Token Reducer that implements ShowUI's reduction algorithms
pub struct TokenReducer {
    config: TokenSelectionConfig,
}

impl TokenReducer {
    /// Creates a new token reducer with the given configuration
    pub fn new(config: &TokenSelectionConfig) -> Result<Self, TokenSelectionError> {
        Ok(Self {
            config: config.clone(),
        })
    }

    /// Reduces tokens based on the RGB connected graph analysis
    pub async fn reduce_tokens(
        &self,
        rgb_graph: &RGBConnectedGraph,
    ) -> Result<(Vec<VisualToken>, TokenReductionStats), String> {
        let original_count = rgb_graph.tokens.len();

        info!(
            "Reducing {} tokens with target reduction: {:.1}%",
            original_count,
            self.config.token_reduction.target_reduction_percentage * 100.0
        );

        // Step 1: Preserve critical tokens (interactive and text elements)
        let mut preserved_tokens = self.preserve_critical_tokens(&rgb_graph.tokens).await?;
        debug!("Preserved {} critical tokens", preserved_tokens.len());

        // Step 2: Reduce redundant tokens using connected graph information
        let reduced_tokens = self
            .reduce_redundant_tokens(
                &mut preserved_tokens,
                &rgb_graph.redundancy_groups,
                &rgb_graph.connections,
            )
            .await?;
        debug!(
            "After redundancy reduction: {} tokens",
            reduced_tokens.len()
        );

        // Step 3: Apply background simplification
        let simplified_tokens = self.simplify_background_tokens(reduced_tokens).await?;
        debug!(
            "After background simplification: {} tokens",
            simplified_tokens.len()
        );

        // Step 4: Final importance-based filtering
        let final_tokens = self.apply_importance_filtering(simplified_tokens).await?;

        let final_count = final_tokens.len();
        let stats = TokenReductionStats::new(original_count as u32, final_count as u32);

        info!(
            "Token reduction complete: {} -> {} tokens ({:.1}% reduction)",
            original_count, final_count, stats.reduction_percentage
        );

        Ok((final_tokens, stats))
    }

    /// Preserves critical tokens that should not be reduced
    async fn preserve_critical_tokens(
        &self,
        tokens: &[VisualToken],
    ) -> Result<Vec<VisualToken>, String> {
        let mut preserved = Vec::new();

        for token in tokens {
            let should_preserve = match token.token_type {
                TokenType::Interactive
                    if self.config.token_reduction.preserve_interactive_elements =>
                {
                    true
                }
                TokenType::Text if self.config.token_reduction.preserve_text_elements => true,
                _ => {
                    // Preserve tokens with high importance regardless of type
                    token.importance_score >= self.config.token_reduction.min_importance_threshold
                }
            };

            if should_preserve {
                preserved.push(token.clone());
            }
        }

        Ok(preserved)
    }

    /// Reduces redundant tokens using connected graph information
    async fn reduce_redundant_tokens(
        &self,
        tokens: &mut [VisualToken],
        redundancy_groups: &[Vec<u32>],
        _connections: &[Vec<bool>],
    ) -> Result<Vec<VisualToken>, String> {
        if !self.config.token_reduction.enable_redundancy_grouping {
            return Ok(tokens.to_vec());
        }

        let mut reduced_tokens = Vec::new();
        let mut processed_indices = std::collections::HashSet::new();

        // Process redundancy groups
        for group in redundancy_groups {
            if group.len() <= 1 {
                continue;
            }

            // Find the most representative token in the group
            let representative_idx = self.find_representative_token(tokens, group)?;

            if let Some(token_idx) = representative_idx {
                if token_idx < tokens.len() {
                    let mut representative_token = tokens[token_idx].clone();

                    // Update the representative token with group information
                    representative_token.redundancy_group = Some(group.len() as u32);
                    representative_token.connected_tokens = group.clone();

                    reduced_tokens.push(representative_token);

                    // Mark all tokens in this group as processed
                    for &idx in group {
                        processed_indices.insert(idx as usize);
                    }
                }
            }
        }

        // Add tokens that weren't part of any redundancy group
        for (idx, token) in tokens.iter().enumerate() {
            if !processed_indices.contains(&idx) {
                reduced_tokens.push(token.clone());
            }
        }

        Ok(reduced_tokens)
    }

    /// Finds the most representative token in a redundancy group
    fn find_representative_token(
        &self,
        tokens: &[VisualToken],
        group: &[u32],
    ) -> Result<Option<usize>, String> {
        if group.is_empty() {
            return Ok(None);
        }

        let mut best_idx = None;
        let mut best_score = f32::NEG_INFINITY;

        for &token_idx in group {
            if token_idx as usize >= tokens.len() {
                continue;
            }

            let token = &tokens[token_idx as usize];

            // Calculate representativeness score
            let mut score = token.importance_score;

            // Prefer interactive and text elements
            match token.token_type {
                TokenType::Interactive => score += 0.3,
                TokenType::Text => score += 0.2,
                TokenType::Chrome => score += 0.1,
                TokenType::Background => score -= 0.1,
                TokenType::Unknown => score += 0.0,
            }

            // Prefer tokens with higher color variance (more detail)
            score += token.color_variance * 0.2;

            if score > best_score {
                best_score = score;
                best_idx = Some(token_idx as usize);
            }
        }

        Ok(best_idx)
    }

    /// Simplifies background tokens based on configuration
    async fn simplify_background_tokens(
        &self,
        tokens: Vec<VisualToken>,
    ) -> Result<Vec<VisualToken>, String> {
        let simplification_level = self.config.token_reduction.background_simplification_level;

        if simplification_level == 0 {
            return Ok(tokens);
        }

        // Calculate how aggressively to simplify based on level (0-3)
        let reduction_factor = match simplification_level {
            1 => 0.8, // Keep 80% of background tokens
            2 => 0.6, // Keep 60% of background tokens
            3 => 0.4, // Keep 40% of background tokens
            _ => 1.0, // Keep all tokens
        };

        // Separate background tokens from others
        let mut background_tokens: Vec<_> = tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| matches!(token.token_type, TokenType::Background))
            .collect();

        let non_background_tokens: Vec<_> = tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| !matches!(token.token_type, TokenType::Background))
            .map(|(_, token)| token.clone())
            .collect();

        // Sort background tokens by importance (keep the most important ones)
        background_tokens.sort_by(|a, b| {
            b.1.importance_score
                .partial_cmp(&a.1.importance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Keep only the top fraction of background tokens
        let keep_count = (background_tokens.len() as f32 * reduction_factor).ceil() as usize;
        let kept_background_tokens: Vec<_> = background_tokens
            .into_iter()
            .take(keep_count)
            .map(|(_, token)| token.clone())
            .collect();

        // Combine non-background and kept background tokens
        let mut result_tokens = non_background_tokens;
        result_tokens.extend(kept_background_tokens);

        debug!(
            "Background simplification (level {}): kept {} background tokens",
            simplification_level, keep_count
        );

        Ok(result_tokens)
    }

    /// Applies final importance-based filtering to meet target reduction
    async fn apply_importance_filtering(
        &self,
        mut tokens: Vec<VisualToken>,
    ) -> Result<Vec<VisualToken>, String> {
        let current_count = tokens.len();
        let target_reduction = self.config.token_reduction.target_reduction_percentage;

        // Calculate how many tokens we need to remove to reach target
        let target_count = (current_count as f32 * (1.0 - target_reduction)).ceil() as usize;

        if current_count <= target_count {
            // Already at or below target, no further reduction needed
            return Ok(tokens);
        }

        // Sort tokens by importance (descending)
        tokens.sort_by(|a, b| {
            b.importance_score
                .partial_cmp(&a.importance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Keep only the most important tokens
        tokens.truncate(target_count);

        debug!(
            "Importance filtering: reduced from {} to {} tokens to meet target reduction",
            current_count, target_count
        );

        Ok(tokens)
    }

    /// Validates the reduction results
    pub fn validate_reduction(
        &self,
        original_tokens: &[VisualToken],
        reduced_tokens: &[VisualToken],
    ) -> Result<bool, String> {
        let original_count = original_tokens.len();
        let reduced_count = reduced_tokens.len();

        // Check that we actually reduced tokens
        if reduced_count >= original_count {
            warn!(
                "Token reduction did not reduce token count: {} -> {}",
                original_count, reduced_count
            );
            return Ok(false);
        }

        // Check that important tokens are preserved
        let original_interactive_count = original_tokens
            .iter()
            .filter(|t| matches!(t.token_type, TokenType::Interactive))
            .count();
        let reduced_interactive_count = reduced_tokens
            .iter()
            .filter(|t| matches!(t.token_type, TokenType::Interactive))
            .count();

        if self.config.token_reduction.preserve_interactive_elements
            && reduced_interactive_count < original_interactive_count
        {
            debug!(
                "Some interactive elements were reduced: {} -> {}",
                original_interactive_count, reduced_interactive_count
            );
        }

        // Check that reduction is within reasonable bounds
        let reduction_percentage =
            ((original_count - reduced_count) as f32 / original_count as f32) * 100.0;
        let target_percentage = self.config.token_reduction.target_reduction_percentage * 100.0;

        if reduction_percentage < target_percentage * 0.5 {
            warn!(
                "Reduction percentage ({:.1}%) is much lower than target ({:.1}%)",
                reduction_percentage, target_percentage
            );
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tools::ui_token_selector::{config::TokenSelectionConfig, RGBColor};

    fn create_test_token(token_type: TokenType, importance: f32) -> VisualToken {
        VisualToken {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
            dominant_color: RGBColor {
                r: 100,
                g: 100,
                b: 100,
            },
            color_variance: 0.5,
            importance_score: importance,
            token_type,
            connected_tokens: Vec::new(),
            redundancy_group: None,
        }
    }

    #[test]
    fn test_token_reducer_creation() {
        let config = TokenSelectionConfig::default();
        let reducer = TokenReducer::new(&config);
        assert!(reducer.is_ok());
    }

    #[tokio::test]
    async fn test_preserve_critical_tokens() {
        let config = TokenSelectionConfig::default();
        let reducer = TokenReducer::new(&config).unwrap();

        let tokens = vec![
            create_test_token(TokenType::Interactive, 0.8),
            create_test_token(TokenType::Text, 0.7),
            create_test_token(TokenType::Background, 0.1),
            create_test_token(TokenType::Chrome, 0.3),
        ];

        let preserved = reducer.preserve_critical_tokens(&tokens).await.unwrap();

        // Should preserve interactive and text tokens
        assert!(preserved.len() >= 2);
        assert!(preserved
            .iter()
            .any(|t| matches!(t.token_type, TokenType::Interactive)));
        assert!(preserved
            .iter()
            .any(|t| matches!(t.token_type, TokenType::Text)));
    }

    #[test]
    fn test_find_representative_token() {
        let config = TokenSelectionConfig::default();
        let reducer = TokenReducer::new(&config).unwrap();

        let tokens = vec![
            create_test_token(TokenType::Background, 0.2),
            create_test_token(TokenType::Interactive, 0.8),
            create_test_token(TokenType::Text, 0.6),
        ];

        let group = vec![0, 1, 2];
        let representative = reducer.find_representative_token(&tokens, &group).unwrap();

        // Should select the interactive token (highest importance + bonus)
        assert_eq!(representative, Some(1));
    }

    #[tokio::test]
    async fn test_simplify_background_tokens() {
        let mut config = TokenSelectionConfig::default();
        config.token_reduction.background_simplification_level = 2; // Keep 60%
        let reducer = TokenReducer::new(&config).unwrap();

        let tokens = vec![
            create_test_token(TokenType::Interactive, 0.8),
            create_test_token(TokenType::Background, 0.3),
            create_test_token(TokenType::Background, 0.2),
            create_test_token(TokenType::Background, 0.1),
            create_test_token(TokenType::Text, 0.7),
        ];

        let simplified = reducer.simplify_background_tokens(tokens).await.unwrap();

        // Should keep all non-background tokens + some background tokens
        let non_background_count = simplified
            .iter()
            .filter(|t| !matches!(t.token_type, TokenType::Background))
            .count();
        assert_eq!(non_background_count, 2); // Interactive + Text

        let background_count = simplified
            .iter()
            .filter(|t| matches!(t.token_type, TokenType::Background))
            .count();
        assert!(background_count <= 2); // Should reduce 3 background tokens
    }

    #[test]
    fn test_validate_reduction() {
        let config = TokenSelectionConfig::default();
        let reducer = TokenReducer::new(&config).unwrap();

        let original = vec![
            create_test_token(TokenType::Interactive, 0.8),
            create_test_token(TokenType::Background, 0.2),
            create_test_token(TokenType::Background, 0.1),
        ];

        let reduced = vec![create_test_token(TokenType::Interactive, 0.8)];

        let is_valid = reducer.validate_reduction(&original, &reduced).unwrap();
        assert!(is_valid);
    }
}
