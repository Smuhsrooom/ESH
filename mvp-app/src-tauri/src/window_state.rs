use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use tauri::{PhysicalPosition, PhysicalSize, Window};

const FILE_NAME: &str = "window-state.json";

#[derive(Deserialize, Serialize)]
pub struct SavedWindow {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

pub fn capture(window: &Window) -> Option<SavedWindow> {
    let position = window.outer_position().ok()?;
    let size = window.inner_size().ok()?;
    Some(SavedWindow {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized: window.is_maximized().ok()?,
    })
}

pub fn save(data_dir: &Path, state: &SavedWindow) -> std::io::Result<()> {
    fs::create_dir_all(data_dir)?;
    let path = data_dir.join(FILE_NAME);
    let temp = data_dir.join("window-state.json.tmp");
    let bytes = serde_json::to_vec(state).map_err(std::io::Error::other)?;
    fs::write(&temp, bytes)?;
    fs::rename(temp, path)
}

pub fn restore(window: &tauri::WebviewWindow, data_dir: &Path) {
    let saved = fs::read(data_dir.join(FILE_NAME))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<SavedWindow>(&bytes).ok());
    if let Some(saved) = saved {
        if saved.maximized {
            let _ = window.maximize();
        } else if saved.width >= 900 && saved.height >= 600 && saved.width <= 10_000 && saved.height <= 10_000 {
            let monitors = window.available_monitors().unwrap_or_default();
            let visible = monitors.iter().any(|monitor| {
                let pos = monitor.position();
                let size = monitor.size();
                saved.x < pos.x.saturating_add(size.width as i32).saturating_sub(100)
                    && saved.x.saturating_add(saved.width as i32) > pos.x.saturating_add(100)
                    && saved.y < pos.y.saturating_add(size.height as i32).saturating_sub(100)
                    && saved.y.saturating_add(saved.height as i32) > pos.y.saturating_add(100)
            });
            let _ = window.set_size(PhysicalSize::new(saved.width, saved.height));
            if visible {
                let _ = window.set_position(PhysicalPosition::new(saved.x, saved.y));
            } else {
                let _ = window.center();
            }
        }
    }
    let _ = window.show();
}
