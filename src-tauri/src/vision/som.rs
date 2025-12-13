// Set-of-Mark (SOM) Visual Grounding System
// Similar to CUA's YOLO + EasyOCR implementation

use anyhow::Result;
use image::{DynamicImage, GenericImageView};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SOMResult {
    pub elements: Vec<UIElement>,
    pub text_regions: Vec<TextRegion>,
    pub annotated_image: Option<Vec<u8>>,
    pub metadata: SOMMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIElement {
    pub id: usize,
    pub element_type: ElementType,
    pub bbox: BoundingBox,
    pub confidence: f32,
    pub label: Option<String>,
    pub clickable: bool,
    pub text_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ElementType {
    Button,
    TextField,
    Label,
    Icon,
    Menu,
    Window,
    Dialog,
    Checkbox,
    RadioButton,
    Slider,
    ScrollBar,
    Image,
    Link,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl BoundingBox {
    pub fn center(&self) -> (u32, u32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
    
    pub fn contains_point(&self, x: u32, y: u32) -> bool {
        x >= self.x && x <= self.x + self.width &&
        y >= self.y && y <= self.y + self.height
    }
    
    pub fn intersection(&self, other: &BoundingBox) -> Option<BoundingBox> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);
        
        if x2 > x1 && y2 > y1 {
            Some(BoundingBox {
                x: x1,
                y: y1,
                width: x2 - x1,
                height: y2 - y1,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRegion {
    pub text: String,
    pub bbox: BoundingBox,
    pub confidence: f32,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SOMMetadata {
    pub processing_time_ms: u64,
    pub image_size: (u32, u32),
    pub model_used: String,
    pub timestamp: std::time::SystemTime,
}

pub struct SOMProcessor {
    yolo_model: Option<YOLOModel>,
    ocr_engine: Option<OCREngine>,
    icon_detector: Option<IconDetector>,
    use_gpu: bool,
}

impl SOMProcessor {
    pub async fn new(use_gpu: bool) -> Result<Self> {
        let yolo_model = YOLOModel::load(use_gpu).await.ok();
        let ocr_engine = OCREngine::new(use_gpu).await.ok();
        let icon_detector = IconDetector::new().await.ok();
        
        Ok(Self {
            yolo_model,
            ocr_engine,
            icon_detector,
            use_gpu,
        })
    }
    
    pub async fn process_screenshot(&self, image: &DynamicImage) -> Result<SOMResult> {
        let start_time = std::time::Instant::now();
        let (width, height) = image.dimensions();
        
        // Run detection models in parallel
        let (elements, text_regions) = tokio::join!(
            self.detect_ui_elements(image),
            self.extract_text(image)
        );
        
        let elements = elements?;
        let text_regions = text_regions?;
        
        // Merge text into UI elements
        let elements = self.merge_text_with_elements(elements, &text_regions);
        
        // Generate annotated image
        let annotated_image = self.annotate_image(image, &elements, &text_regions)?;
        
        Ok(SOMResult {
            elements,
            text_regions,
            annotated_image: Some(annotated_image),
            metadata: SOMMetadata {
                processing_time_ms: start_time.elapsed().as_millis() as u64,
                image_size: (width, height),
                model_used: self.get_model_info(),
                timestamp: std::time::SystemTime::now(),
            },
        })
    }
    
    async fn detect_ui_elements(&self, image: &DynamicImage) -> Result<Vec<UIElement>> {
        let mut elements = Vec::new();
        
        // Use YOLO for general UI element detection
        if let Some(yolo) = &self.yolo_model {
            elements.extend(yolo.detect(image).await?);
        }
        
        // Use icon detector for specific icons
        if let Some(icon_detector) = &self.icon_detector {
            elements.extend(icon_detector.detect(image).await?);
        }
        
        // Use accessibility APIs if available (platform-specific)
        #[cfg(target_os = "macos")]
        {
            elements.extend(self.detect_with_accessibility_api().await?);
        }
        
        // Remove duplicates and merge overlapping elements
        elements = self.merge_overlapping_elements(elements);
        
        Ok(elements)
    }
    
    async fn extract_text(&self, image: &DynamicImage) -> Result<Vec<TextRegion>> {
        if let Some(ocr) = &self.ocr_engine {
            ocr.extract_text(image).await
        } else {
            Ok(Vec::new())
        }
    }
    
    fn merge_text_with_elements(
        &self,
        mut elements: Vec<UIElement>,
        text_regions: &[TextRegion]
    ) -> Vec<UIElement> {
        for element in &mut elements {
            // Find text that overlaps with this element
            for text_region in text_regions {
                if let Some(_) = element.bbox.intersection(&text_region.bbox) {
                    element.text_content = Some(text_region.text.clone());
                    if element.label.is_none() {
                        element.label = Some(text_region.text.clone());
                    }
                }
            }
        }
        elements
    }
    
    fn merge_overlapping_elements(&self, elements: Vec<UIElement>) -> Vec<UIElement> {
        // Simple implementation - can be improved with more sophisticated merging
        let mut merged = Vec::new();
        let mut processed = vec![false; elements.len()];
        
        for i in 0..elements.len() {
            if processed[i] {
                continue;
            }
            
            let mut current = elements[i].clone();
            processed[i] = true;
            
            for j in (i + 1)..elements.len() {
                if processed[j] {
                    continue;
                }
                
                if let Some(_) = current.bbox.intersection(&elements[j].bbox) {
                    // Merge if significant overlap
                    // For now, just keep the one with higher confidence
                    if elements[j].confidence > current.confidence {
                        current = elements[j].clone();
                    }
                    processed[j] = true;
                }
            }
            
            merged.push(current);
        }
        
        merged
    }
    
    fn annotate_image(
        &self,
        image: &DynamicImage,
        elements: &[UIElement],
        text_regions: &[TextRegion]
    ) -> Result<Vec<u8>> {
        use image::{Rgba, RgbaImage};
        use imageproc::drawing::{draw_hollow_rect_mut, draw_text_mut};
        use imageproc::rect::Rect;
        use rusttype::{Font, Scale};
        
        let mut annotated = image.to_rgba8();
        
        // Draw bounding boxes for UI elements
        for (i, element) in elements.iter().enumerate() {
            let color = match element.element_type {
                ElementType::Button => Rgba([0, 255, 0, 255]),
                ElementType::TextField => Rgba([0, 0, 255, 255]),
                ElementType::Link => Rgba([255, 0, 255, 255]),
                _ => Rgba([255, 255, 0, 255]),
            };
            
            draw_hollow_rect_mut(
                &mut annotated,
                Rect::at(element.bbox.x as i32, element.bbox.y as i32)
                    .of_size(element.bbox.width, element.bbox.height),
                color
            );
            
            // Draw element ID
            let font_data = include_bytes!("../../assets/fonts/Roboto-Regular.ttf");
            let font = Font::try_from_bytes(font_data as &[u8]).unwrap();
            let scale = Scale::uniform(16.0);
            
            draw_text_mut(
                &mut annotated,
                Rgba([255, 255, 255, 255]),
                element.bbox.x as i32,
                element.bbox.y as i32 - 20,
                scale,
                &font,
                &format!("[{}]", i)
            );
        }
        
        // Convert to bytes
        let mut buffer = Vec::new();
        image::codecs::png::PngEncoder::new(&mut buffer)
            .encode(
                &annotated,
                annotated.width(),
                annotated.height(),
                image::ColorType::Rgba8
            )?;
        
        Ok(buffer)
    }
    
    #[cfg(target_os = "macos")]
    async fn detect_with_accessibility_api(&self) -> Result<Vec<UIElement>> {
        // Use macOS Accessibility API to detect UI elements
        // This provides more accurate detection for native apps
        Ok(Vec::new()) // Placeholder
    }
    
    fn get_model_info(&self) -> String {
        let mut info = Vec::new();
        if self.yolo_model.is_some() {
            info.push("YOLO");
        }
        if self.ocr_engine.is_some() {
            info.push("OCR");
        }
        if self.icon_detector.is_some() {
            info.push("IconDetector");
        }
        info.join("+")
    }
    
    pub fn find_element_by_text(&self, result: &SOMResult, text: &str) -> Option<&UIElement> {
        result.elements.iter().find(|e| {
            e.text_content.as_ref()
                .map(|t| t.contains(text))
                .unwrap_or(false) ||
            e.label.as_ref()
                .map(|l| l.contains(text))
                .unwrap_or(false)
        })
    }
    
    pub fn find_element_by_type(&self, result: &SOMResult, element_type: ElementType) -> Vec<&UIElement> {
        result.elements.iter()
            .filter(|e| std::mem::discriminant(&e.element_type) == std::mem::discriminant(&element_type))
            .collect()
    }
}

// YOLO Model wrapper
struct YOLOModel {
    // In real implementation, this would hold the actual YOLO model
    use_gpu: bool,
}

impl YOLOModel {
    async fn load(use_gpu: bool) -> Result<Self> {
        // Load YOLO model weights
        Ok(Self { use_gpu })
    }
    
    async fn detect(&self, image: &DynamicImage) -> Result<Vec<UIElement>> {
        // Run YOLO inference
        // This is a placeholder - real implementation would use actual YOLO
        Ok(Vec::new())
    }
}

// OCR Engine wrapper
struct OCREngine {
    use_gpu: bool,
}

impl OCREngine {
    async fn new(use_gpu: bool) -> Result<Self> {
        Ok(Self { use_gpu })
    }
    
    async fn extract_text(&self, image: &DynamicImage) -> Result<Vec<TextRegion>> {
        // Run OCR
        // This is a placeholder - real implementation would use EasyOCR or similar
        Ok(Vec::new())
    }
}

// Icon Detector for specific UI icons
struct IconDetector;

impl IconDetector {
    async fn new() -> Result<Self> {
        Ok(Self)
    }
    
    async fn detect(&self, image: &DynamicImage) -> Result<Vec<UIElement>> {
        // Detect specific icons
        Ok(Vec::new())
    }
}