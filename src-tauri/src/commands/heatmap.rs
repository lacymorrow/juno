use tauri::{AppHandle, Emitter, Manager};
use tracing::{info, debug, error};
use std::sync::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapPoint {
    pub x: f64,
    pub y: f64,
    pub timestamp: u64,
    pub intensity: f64, // Weight for this point (clicks have higher intensity)
    pub event_type: String, // "click", "move", "hover"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapData {
    pub points: Vec<HeatmapPoint>,
    pub grid_size: u32,
    pub screen_width: f64,
    pub screen_height: f64,
    pub start_time: u64,
    pub last_update: u64,
}

impl Default for HeatmapData {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            grid_size: 20, // 20x20 pixel grid cells
            screen_width: 1920.0, // Default values, will be updated
            screen_height: 1080.0,
            start_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            last_update: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }
}

pub struct HeatmapTracker {
    pub data: Mutex<HeatmapData>,
    pub is_tracking: Mutex<bool>,
    pub last_position: Mutex<Option<(f64, f64)>>,
}

impl Default for HeatmapTracker {
    fn default() -> Self {
        Self {
            data: Mutex::new(HeatmapData::default()),
            is_tracking: Mutex::new(false),
            last_position: Mutex::new(None),
        }
    }
}

impl HeatmapTracker {
    pub fn add_point(&self, x: f64, y: f64, event_type: &str, intensity: f64) {
        let mut data = self.data.lock().unwrap();
        let is_tracking = *self.is_tracking.lock().unwrap();

        if !is_tracking {
            return;
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let point = HeatmapPoint {
            x,
            y,
            timestamp,
            intensity,
            event_type: event_type.to_string(),
        };

        data.points.push(point);
        data.last_update = timestamp;

        // Limit the number of points to prevent memory issues
        if data.points.len() > 50000 {
            data.points.remove(0);
        }

        debug!("Added heatmap point: ({}, {}) type: {} intensity: {}", x, y, event_type, intensity);
    }

    pub fn start_tracking(&self, screen_width: f64, screen_height: f64) {
        *self.is_tracking.lock().unwrap() = true;
        let mut data = self.data.lock().unwrap();
        data.screen_width = screen_width;
        data.screen_height = screen_height;
        data.start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        info!("Started heatmap tracking with screen dimensions: {}x{}", screen_width, screen_height);
    }

    pub fn stop_tracking(&self) {
        *self.is_tracking.lock().unwrap() = false;
        info!("Stopped heatmap tracking");
    }

    pub fn clear_data(&self) {
        let mut data = self.data.lock().unwrap();
        data.points.clear();
        data.start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        info!("Cleared heatmap data");
    }

    pub fn is_tracking(&self) -> bool {
        *self.is_tracking.lock().unwrap()
    }
}



// Helper function to record click events in heatmap
pub fn record_click_heatmap(app: &AppHandle, x: f64, y: f64, click_type: &str) {
    if let Some(tracker) = app.try_state::<HeatmapTracker>() {
        let intensity = match click_type {
            "left" => 2.0,
            "right" => 1.8,
            "double" => 3.0,
            "triple" => 4.0,
            "middle" => 1.5,
            _ => 1.0,
        };
        tracker.add_point(x, y, "click", intensity);

        // Emit heatmap update event
        if let Err(e) = app.emit("heatmap-point-added", (x, y, intensity, "click")) {
            error!("Failed to emit heatmap point event: {}", e);
        }
    }
}

// Helper function to record mouse movement in heatmap
pub fn record_move_heatmap(app: &AppHandle, x: f64, y: f64) {
    if let Some(tracker) = app.try_state::<HeatmapTracker>() {
        // Only record if we've moved a significant distance to avoid spam
        let mut should_record = true;
        if let Ok(last_pos) = tracker.last_position.lock() {
            if let Some((last_x, last_y)) = *last_pos {
                let distance = ((x - last_x).powi(2) + (y - last_y).powi(2)).sqrt();
                should_record = distance > 10.0; // Only record if moved more than 10 pixels
            }
        }

        if should_record {
            tracker.add_point(x, y, "move", 0.3);
            *tracker.last_position.lock().unwrap() = Some((x, y));

            // Emit heatmap update event (less frequently for moves)
            if let Err(e) = app.emit("heatmap-point-added", (x, y, 0.3, "move")) {
                error!("Failed to emit heatmap move event: {}", e);
            }
        }
    }
}

#[tauri::command]
pub async fn start_heatmap_tracking(
    app: AppHandle,
    screen_width: f64,
    screen_height: f64,
) -> Result<(), String> {
    info!("Starting heatmap tracking with dimensions: {}x{}", screen_width, screen_height);

    if let Some(tracker) = app.try_state::<HeatmapTracker>() {
        tracker.start_tracking(screen_width, screen_height);

        // Emit event to frontend
        app.emit("heatmap-tracking-started", (screen_width, screen_height))
            .map_err(|e| format!("Failed to emit heatmap tracking started event: {}", e))?;

        Ok(())
    } else {
        Err("Heatmap tracker not initialized".to_string())
    }
}

#[tauri::command]
pub async fn stop_heatmap_tracking(app: AppHandle) -> Result<(), String> {
    info!("Stopping heatmap tracking");

    if let Some(tracker) = app.try_state::<HeatmapTracker>() {
        tracker.stop_tracking();

        // Emit event to frontend
        app.emit("heatmap-tracking-stopped", ())
            .map_err(|e| format!("Failed to emit heatmap tracking stopped event: {}", e))?;

        Ok(())
    } else {
        Err("Heatmap tracker not initialized".to_string())
    }
}

#[tauri::command]
pub async fn clear_heatmap_data(app: AppHandle) -> Result<(), String> {
    info!("Clearing heatmap data");

    if let Some(tracker) = app.try_state::<HeatmapTracker>() {
        tracker.clear_data();

        // Emit event to frontend
        app.emit("heatmap-data-cleared", ())
            .map_err(|e| format!("Failed to emit heatmap data cleared event: {}", e))?;

        Ok(())
    } else {
        Err("Heatmap tracker not initialized".to_string())
    }
}

#[tauri::command]
pub async fn get_heatmap_data(app: AppHandle) -> Result<HeatmapData, String> {
    if let Some(tracker) = app.try_state::<HeatmapTracker>() {
        let data = tracker.data.lock().unwrap();
        Ok(data.clone())
    } else {
        Err("Heatmap tracker not initialized".to_string())
    }
}

#[tauri::command]
pub async fn is_heatmap_tracking(app: AppHandle) -> Result<bool, String> {
    if let Some(tracker) = app.try_state::<HeatmapTracker>() {
        Ok(tracker.is_tracking())
    } else {
        Ok(false)
    }
}

// Generate heatmap grid data for visualization
#[tauri::command]
pub async fn get_heatmap_grid(app: AppHandle, grid_size: Option<u32>) -> Result<Vec<Vec<f64>>, String> {
    if let Some(tracker) = app.try_state::<HeatmapTracker>() {
        let data = tracker.data.lock().unwrap();
        let grid_size = grid_size.unwrap_or(data.grid_size);

        let cols = (data.screen_width / grid_size as f64).ceil() as usize;
        let rows = (data.screen_height / grid_size as f64).ceil() as usize;

        let mut grid = vec![vec![0.0; cols]; rows];

        // Aggregate points into grid cells
        for point in &data.points {
            let col = ((point.x / grid_size as f64) as usize).min(cols - 1);
            let row = ((point.y / grid_size as f64) as usize).min(rows - 1);
            grid[row][col] += point.intensity;
        }

        Ok(grid)
    } else {
        Err("Heatmap tracker not initialized".to_string())
    }
}
