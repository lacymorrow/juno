# UI-Guided Visual Token Selection Implementation Plan

**Project**: Juno AI Computer Use Agent - Priority 2 Implementation  
**Status**: 📋 **PLANNING PHASE** - Comprehensive Implementation Strategy  
**Target**: 33% computational cost reduction with multi-monitor support  
**Timeline**: 2-4 weeks implementation  

## 🎯 Executive Summary

This document outlines the comprehensive implementation plan for **UI-Guided Visual Token Selection** in the Juno AI Computer Use Agent, with special focus on **multi-monitor support** and **adaptive display configurations**. The implementation will integrate with our existing multi-monitor screenshot system to provide intelligent visual token reduction while maintaining full functionality across diverse display setups.

For test coverage of this feature inside the broader app automation matrix, see [`docs/rules/ui_test_scenarios.mdc`](./ui_test_scenarios.mdc).

## 🔬 Research Foundation

### Primary Research Source: ShowUI Paper (arXiv:2411.17465)

**Key Findings**:

- **33% reduction in computational costs** while maintaining performance
- **1.4x faster training** through intelligent token selection
- **RGB color space connected graphs** for UI element analysis
- **Adaptive redundancy identification** for sparse UI regions
- **Token reduction**: 1296 → 291 tokens in sparse areas (77% reduction)

### Multi-Monitor Research Context

**Current Juno Capabilities Analysis**:

- ✅ **Multi-Display Detection**: `get_active_displays()` supports up to 32 displays
- ✅ **Display-Aware Screenshots**: `capture_screenshot_cgimage()` with display ID targeting
- ✅ **Coordinate Translation**: Global ↔ display-relative coordinate conversion
- ✅ **Element-Display Mapping**: `find_display_containing_point()` for UI element location
- ✅ **Window-Display Association**: Automatic display detection for windows/elements

## 🏗️ Current Codebase Integration Points

### Screenshot System Architecture

```
Multi-Monitor Screenshot Pipeline:
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│ Cursor Position │───▶│ Display Detection│───▶│ Target Display  │
│ (Global Coords) │    │ find_display_    │    │ Screenshot      │
└─────────────────┘    │ containing_point │    │ Capture         │
                       └──────────────────┘    └─────────────────┘
                                ▼
                       ┌──────────────────┐    ┌─────────────────┐
                       │ Display Bounds   │───▶│ Coordinate      │
                       │ Adjustment       │    │ Translation     │
                       └──────────────────┘    └─────────────────┘
```

### Key Integration Files

1. **`src-tauri/src/agent/tools/anthropic_computer_use.rs`** (998 lines)
   - Primary implementation target
   - Screenshot action handling (lines 246-275)
   - Multi-display coordinate handling

2. **`src-tauri/mcp-server-os-level/src/platforms/macos/utils.rs`** (693 lines)
   - `capture_and_encode_screenshot()` - main display detection
   - `find_display_containing_point()` - display selection logic
   - Multi-monitor screenshot capture infrastructure

3. **`src-tauri/mcp-server-os-level/src/platforms/macos/display.rs`** (222 lines)
   - `get_active_displays()` - comprehensive display enumeration
   - `DisplayInfo` struct with bounds and metadata
   - Multi-monitor coordinate adjustment functions

4. **`src-tauri/src/utils/coordinates.rs`** (91 lines)
   - Screenshot scaling information management
   - Coordinate transformation utilities

## 🎨 UI-Guided Visual Token Selection Architecture

### Core Algorithm Implementation

```rust
pub struct UIGuidedTokenSelector {
    // Multi-monitor configuration
    display_info: Vec<DisplayInfo>,
    active_display_id: CGDirectDisplayID,
    
    // Visual analysis components
    rgb_graph_analyzer: RGBConnectedGraphAnalyzer,
    redundancy_detector: VisualRedundancyDetector,
    ui_element_prioritizer: UIElementPrioritizer,
    
    // Performance optimization
    token_reduction_cache: LRUCache<String, TokenReductionResult>,
    display_specific_cache: HashMap<CGDirectDisplayID, DisplayCache>,
}

pub struct TokenReductionResult {
    original_tokens: usize,
    reduced_tokens: usize,
    reduction_percentage: f32,
    preserved_regions: Vec<CriticalUIRegion>,
    redundant_regions: Vec<RedundantRegion>,
}
```

### Multi-Monitor Adaptation Strategy

#### 1. Display-Aware Token Selection

```rust
impl UIGuidedTokenSelector {
    pub async fn process_multi_monitor_screenshot(
        &mut self,
        screenshot_data: &str,
        target_display_id: Option<CGDirectDisplayID>
    ) -> Result<TokenReductionResult, AgentError> {
        // 1. Determine active display
        let display_id = self.resolve_target_display(target_display_id).await?;
        
        // 2. Get display-specific configuration
        let display_config = self.get_display_configuration(display_id)?;
        
        // 3. Apply display-aware token selection
        let token_result = self.apply_display_optimized_selection(
            screenshot_data,
            &display_config
        ).await?;
        
        // 4. Cache results per display
        self.cache_display_result(display_id, &token_result);
        
        Ok(token_result)
    }
}
```

#### 2. Display Configuration Adaptation

```rust
#[derive(Debug, Clone)]
pub struct DisplayConfiguration {
    pub display_id: CGDirectDisplayID,
    pub resolution: (u32, u32),
    pub scale_factor: f64,
    pub orientation: DisplayOrientation,
    pub ui_density: UIDensityProfile,
    pub layout_type: DisplayLayoutType,
}

#[derive(Debug, Clone)]
pub enum DisplayOrientation {
    Landscape,
    Portrait,
    LandscapeFlipped,
    PortraitFlipped,
}

#[derive(Debug, Clone)]
pub enum DisplayLayoutType {
    Primary,
    Extended,
    Mirrored,
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone)]
pub struct UIDensityProfile {
    pub sparse_regions_threshold: f32,
    pub ui_element_density: f32,
    pub expected_token_reduction: f32,
}
```

### RGB Connected Graph Analysis

#### Core Visual Processing Pipeline

```rust
pub struct RGBConnectedGraphAnalyzer {
    color_similarity_threshold: f32,
    spatial_connectivity_radius: u32,
    ui_element_detection_weights: UIElementWeights,
}

impl RGBConnectedGraphAnalyzer {
    pub fn analyze_screenshot_regions(
        &self,
        image_buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        display_config: &DisplayConfiguration
    ) -> Result<ConnectedGraphResult, AnalysisError> {
        // 1. Convert to RGB color space connected graph
        let rgb_graph = self.build_rgb_connectivity_graph(image_buffer)?;
        
        // 2. Identify visually redundant patches
        let redundant_patches = self.identify_redundant_patches(
            &rgb_graph,
            display_config.ui_density.sparse_regions_threshold
        )?;
        
        // 3. Preserve functionally critical elements
        let critical_elements = self.detect_critical_ui_elements(
            &rgb_graph,
            &display_config
        )?;
        
        // 4. Generate token selection strategy
        let token_strategy = self.generate_token_selection_strategy(
            &redundant_patches,
            &critical_elements
        )?;
        
        Ok(ConnectedGraphResult {
            rgb_graph,
            redundant_patches,
            critical_elements,
            token_strategy,
        })
    }
}
```

### Multi-Monitor Token Selection Strategy

#### Display Layout Optimization

```rust
pub struct MultiMonitorTokenOptimizer {
    layout_detector: DisplayLayoutDetector,
    cross_display_analyzer: CrossDisplayAnalyzer,
}

impl MultiMonitorTokenOptimizer {
    pub fn optimize_across_displays(
        &self,
        display_screenshots: Vec<(CGDirectDisplayID, String)>
    ) -> Result<MultiDisplayTokenResult, OptimizerError> {
        // 1. Analyze display layout configuration
        let layout_config = self.layout_detector.analyze_display_layout(
            &display_screenshots
        )?;
        
        // 2. Apply layout-specific optimizations
        match layout_config.layout_type {
            DisplayLayoutPattern::HorizontalExtended => {
                self.optimize_horizontal_layout(&display_screenshots)
            },
            DisplayLayoutPattern::VerticalExtended => {
                self.optimize_vertical_layout(&display_screenshots)
            },
            DisplayLayoutPattern::Mixed => {
                self.optimize_mixed_layout(&display_screenshots)
            },
            DisplayLayoutPattern::Single => {
                self.optimize_single_display(&display_screenshots[0])
            }
        }
    }
}
```

## 🔧 Implementation Phases

### Phase 1: Core Infrastructure (Week 1)

#### 1.1 Base Token Selection System

- [ ] **Create `UIGuidedTokenSelector` struct** in `anthropic_computer_use.rs`
- [ ] **Implement RGB connected graph analyzer**
- [ ] **Add visual redundancy detection algorithms**
- [ ] **Create token reduction result structures**

#### 1.2 Multi-Monitor Integration

- [ ] **Extend existing display detection system**
- [ ] **Add display configuration analysis**
- [ ] **Implement display-specific caching**
- [ ] **Create coordinate translation utilities**

#### 1.3 Performance Optimization Foundation

- [ ] **Add LRU cache for token reduction results**
- [ ] **Implement display-specific optimization profiles**
- [ ] **Create performance metrics collection**

### Phase 2: Algorithm Implementation (Week 2)

#### 2.1 RGB Graph Analysis

- [ ] **Implement color similarity clustering**
- [ ] **Add spatial connectivity analysis**
- [ ] **Create UI element detection weights**
- [ ] **Build redundant patch identification**

#### 2.2 Critical Element Preservation

- [ ] **Implement interactive element detection**
- [ ] **Add text region preservation logic**
- [ ] **Create navigation element prioritization**
- [ ] **Build accessibility element protection**

#### 2.3 Token Selection Strategy

- [ ] **Implement adaptive token reduction**
- [ ] **Add display-aware thresholds**
- [ ] **Create orientation-specific optimizations**
- [ ] **Build density-based selection criteria**

### Phase 3: Multi-Monitor Optimization (Week 3)

#### 3.1 Display Layout Detection

- [ ] **Implement layout pattern recognition**
- [ ] **Add orientation detection**
- [ ] **Create resolution adaptation**
- [ ] **Build scale factor handling**

#### 3.2 Cross-Display Analysis

- [ ] **Implement multi-display coordination**
- [ ] **Add layout-specific optimizations**
- [ ] **Create display priority systems**
- [ ] **Build unified token strategies**

#### 3.3 Performance Validation

- [ ] **Measure computational cost reduction**
- [ ] **Validate 33% performance improvement**
- [ ] **Test across different display configurations**
- [ ] **Benchmark against baseline performance**

### Phase 4: Integration & Testing (Week 4)

#### 4.1 Computer Use Tool Integration

- [ ] **Integrate with screenshot action**
- [ ] **Add optional token selection parameter**
- [ ] **Implement backward compatibility**
- [ ] **Create configuration options**

#### 4.2 Comprehensive Testing

- [ ] **Test single monitor configurations**
- [ ] **Test dual monitor setups (horizontal/vertical)**
- [ ] **Test triple+ monitor configurations**
- [ ] **Test mixed resolution scenarios**
- [ ] **Test different orientations**

#### 4.3 Documentation & Optimization

- [ ] **Update research documentation**
- [ ] **Create performance benchmarks**
- [ ] **Add configuration guides**
- [ ] **Optimize based on test results**

## 📊 Expected Performance Improvements

### Computational Cost Reduction

| Display Configuration | Expected Token Reduction | Computational Savings | Performance Gain |
|----------------------|--------------------------|----------------------|------------------|
| Single 4K Display    | 65-75%                  | 30-35%               | 1.3-1.4x         |
| Dual HD Displays     | 70-80%                  | 35-40%               | 1.4-1.5x         |
| Triple+ Displays     | 75-85%                  | 40-45%               | 1.5-1.6x         |
| Mixed Resolutions    | 60-70%                  | 25-30%               | 1.2-1.3x         |

### Multi-Monitor Specific Benefits

1. **Display-Aware Processing**: Only process active display screenshots
2. **Layout Optimization**: Optimize token selection based on display arrangement
3. **Redundancy Elimination**: Remove duplicate UI elements across displays
4. **Priority Focusing**: Focus computational resources on primary interaction display

## 🔍 Technical Implementation Details

### Screenshot Processing Pipeline Enhancement

```rust
// Enhanced screenshot action in anthropic_computer_use.rs
"screenshot" => {
    let window_id = input["window_id"].as_str();
    let use_focused_window = input["use_focused_window"].as_bool().unwrap_or(false);
    let enable_token_selection = input["enable_token_selection"].as_bool().unwrap_or(true);
    
    // Existing screenshot capture logic...
    let screenshot_result = /* existing logic */;
    
    match screenshot_result {
        Ok(base64_image) => {
            let final_image = if enable_token_selection {
                // NEW: Apply UI-Guided Visual Token Selection
                let token_selector = app.state::<UIGuidedTokenSelector>();
                match token_selector.process_screenshot(&base64_image).await {
                    Ok(optimized_result) => {
                        info!("Token selection applied: {}% reduction", 
                              optimized_result.reduction_percentage);
                        optimized_result.optimized_image
                    },
                    Err(e) => {
                        warn!("Token selection failed, using original: {}", e);
                        base64_image
                    }
                }
            } else {
                base64_image
            };
            
            Ok(json!({
                "type": "image",
                "data": final_image,
                "format": "png",
                "token_optimization": enable_token_selection
            }))
        },
        Err(e) => Err(format!("Screenshot failed: {}", e))
    }
}
```

### Display Configuration Detection

```rust
impl DisplayConfiguration {
    pub fn detect_from_display_info(display_info: &DisplayInfo) -> Self {
        let resolution = (
            display_info.bounds.size.width as u32,
            display_info.bounds.size.height as u32
        );
        
        let orientation = if resolution.0 > resolution.1 {
            DisplayOrientation::Landscape
        } else {
            DisplayOrientation::Portrait
        };
        
        let ui_density = UIDensityProfile::calculate_for_resolution(resolution);
        
        DisplayConfiguration {
            display_id: display_info.id,
            resolution,
            scale_factor: Self::detect_scale_factor(&display_info),
            orientation,
            ui_density,
            layout_type: DisplayLayoutType::detect_layout_type(&display_info),
        }
    }
}
```

## 🧪 Testing Strategy

### Multi-Monitor Test Scenarios

#### Scenario 1: Horizontal Dual Monitor

- **Configuration**: Two 1920x1080 displays side-by-side
- **Expected**: 70% token reduction, 35% computational savings
- **Test Focus**: Cross-display UI element detection

#### Scenario 2: Vertical Dual Monitor

- **Configuration**: Two displays stacked vertically
- **Expected**: 75% token reduction, 38% computational savings
- **Test Focus**: Vertical layout optimization

#### Scenario 3: Mixed Resolution Triple

- **Configuration**: 4K primary + two 1080p secondary displays
- **Expected**: 80% token reduction, 42% computational savings
- **Test Focus**: Resolution-adaptive token selection

#### Scenario 4: Portrait + Landscape Mixed

- **Configuration**: Portrait display + landscape display
- **Expected**: 65% token reduction, 30% computational savings
- **Test Focus**: Orientation-specific optimization

### Performance Benchmarking

```rust
#[cfg(test)]
mod performance_tests {
    use super::*;
    
    #[tokio::test]
    async fn benchmark_single_display_token_reduction() {
        let selector = UIGuidedTokenSelector::new();
        let screenshot = capture_test_screenshot().await;
        
        let start = Instant::now();
        let result = selector.process_screenshot(&screenshot).await.unwrap();
        let duration = start.elapsed();
        
        assert!(result.reduction_percentage >= 30.0);
        assert!(duration.as_millis() < 500); // Must complete within 500ms
    }
    
    #[tokio::test]
    async fn benchmark_multi_display_coordination() {
        let optimizer = MultiMonitorTokenOptimizer::new();
        let screenshots = capture_multi_display_screenshots().await;
        
        let start = Instant::now();
        let result = optimizer.optimize_across_displays(screenshots).await.unwrap();
        let duration = start.elapsed();
        
        assert!(result.overall_reduction >= 35.0);
        assert!(duration.as_millis() < 1000); // Must complete within 1s
    }
}
```

## 📚 Research Citations & References

### Primary Research Sources

1. **ShowUI: UI-Guided Visual Token Selection (arXiv:2411.17465)**
   - **Key Contribution**: 33% computational cost reduction through RGB connected graphs
   - **Implementation**: Visual redundancy detection, UI element prioritization
   - **Performance**: 1.4x faster training, 77% token reduction in sparse areas

2. **GUI-Actor: Coordinate-Free Visual Grounding**
   - **Key Contribution**: Visual grounding techniques for GUI automation
   - **Implementation**: Element detection without coordinate dependencies
   - **Performance**: Improved accuracy in multi-resolution scenarios

3. **Aria-UI: Visual Grounding in GUI Instructions**
   - **Key Contribution**: UI element understanding and prioritization
   - **Implementation**: Context-aware element importance scoring
   - **Performance**: Enhanced UI element detection accuracy

### Multi-Monitor Research Context

4. **macOS Multi-Display Management (Apple Developer Documentation)**
   - **Key Contribution**: Display enumeration and coordinate systems
   - **Implementation**: CGDirectDisplayID management, bounds calculation
   - **Performance**: Efficient multi-display screenshot capture

5. **Computer Vision for GUI Automation (Industrial Applications)**
   - **Key Contribution**: Adaptive visual processing for different display configurations
   - **Implementation**: Resolution-aware image processing, layout detection
   - **Performance**: Scalable processing across display configurations

## 🎯 Success Metrics & Validation

### Primary Success Criteria

1. **Computational Cost Reduction**: ≥33% reduction in processing costs
2. **Performance Improvement**: ≥1.4x faster processing speed
3. **Multi-Monitor Support**: Full functionality across all display configurations
4. **Accuracy Preservation**: No reduction in UI interaction accuracy

### Secondary Success Criteria

1. **Memory Efficiency**: Reduced memory usage for screenshot processing
2. **Cache Effectiveness**: >80% cache hit rate for similar screenshots
3. **Display Adaptation**: Automatic optimization for different display layouts
4. **Backward Compatibility**: Seamless integration with existing functionality

### Validation Methods

1. **Automated Benchmarking**: Continuous performance measurement
2. **Multi-Display Testing**: Comprehensive testing across display configurations
3. **User Acceptance Testing**: Real-world usage validation
4. **Research Validation**: Comparison with ShowUI paper results

## 🔄 Maintenance & Future Enhancements

### Ongoing Maintenance

- **Performance Monitoring**: Continuous tracking of token reduction effectiveness
- **Display Configuration Updates**: Adaptation to new display technologies
- **Algorithm Refinement**: Iterative improvement based on usage patterns
- **Cache Optimization**: Regular cache performance analysis and tuning

### Future Enhancement Opportunities

1. **Machine Learning Integration**: Learn optimal token selection patterns
2. **Dynamic Threshold Adjustment**: Adaptive optimization based on usage patterns
3. **Cross-Platform Extension**: Extend to Windows and Linux multi-monitor support
4. **Real-Time Optimization**: Live adjustment during agent execution

---

**Document Version**: 1.0  
**Last Updated**: January 2025  
**Implementation Target**: February 2025  
**Review Cycle**: Weekly during implementation, monthly post-deployment
