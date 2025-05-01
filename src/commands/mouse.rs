use juno_lib::models::{RequestData, ResponseData};
use juno_lib::platform::PlatformMouseTrait;
use juno_lib::state::AppState;
use log::{error, info};
use juno_lib::utils::coordinates;

#[tauri::command]
pub fn mouse_move(x: f64, y: f64, state: tauri::State<AppState>) -> ResponseData<()> {
    info!("Moving mouse to ({}, {})", x, y);
    let platform_mouse = state.platform_mouse.lock().unwrap();
    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);
    match platform_mouse.move_mouse(screen_x, screen_y) {
        Ok(_) => ResponseData::new_success("Mouse moved successfully".to_string(), None),
        Err(e) => {
            error!("Failed to move mouse: {}", e);
            ResponseData::new_error("Failed to move mouse".to_string(), None)
        }
    }
}
