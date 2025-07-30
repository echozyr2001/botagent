pub mod applications;
pub mod files;
pub mod keyboard;
pub mod mouse;
pub mod screen;

use async_trait::async_trait;
use bytebot_shared_rs::types::computer_action::{
    Application, Button, ComputerAction, Coordinates, Press, ScrollDirection,
};
use serde_json::Value;

use crate::error::AutomationError;

/// Main automation service that coordinates all desktop automation operations
#[derive(Debug, Clone)]
pub struct AutomationService {
    pub screen: screen::ScreenService,
    pub mouse: mouse::MouseService,
    pub keyboard: keyboard::KeyboardService,
    pub applications: applications::ApplicationService,
    pub files: files::FileService,
}

impl AutomationService {
    pub fn new() -> Result<Self, AutomationError> {
        Ok(Self {
            screen: screen::ScreenService::new()?,
            mouse: mouse::MouseService::new()?,
            keyboard: keyboard::KeyboardService::new()?,
            applications: applications::ApplicationService::new()?,
            files: files::FileService::new()?,
        })
    }

    /// Execute a computer action and return the result as JSON
    pub async fn execute_action(&self, action: ComputerAction) -> Result<Value, AutomationError> {
        match action {
            ComputerAction::MoveMouse { coordinates } => {
                self.move_mouse(coordinates).await?;
                Ok(serde_json::json!({"success": true}))
            }

            ComputerAction::TraceMouse { path, hold_keys } => {
                self.trace_mouse_path(&path, hold_keys.as_deref()).await?;
                Ok(serde_json::json!({"success": true}))
            }

            ComputerAction::ClickMouse {
                coordinates,
                button,
                hold_keys,
                click_count,
            } => {
                self.click_mouse_with_options(
                    coordinates,
                    button,
                    click_count,
                    hold_keys.as_deref(),
                )
                .await?;
                Ok(serde_json::json!({"success": true}))
            }

            ComputerAction::PressMouse {
                coordinates,
                button,
                press,
            } => {
                self.press_mouse(coordinates, button, press).await?;
                Ok(serde_json::json!({"success": true}))
            }

            ComputerAction::DragMouse {
                path,
                button,
                hold_keys,
            } => {
                self.drag_mouse_path(&path, button, hold_keys.as_deref())
                    .await?;
                Ok(serde_json::json!({"success": true}))
            }

            ComputerAction::Scroll {
                coordinates,
                direction,
                scroll_count,
                hold_keys,
            } => {
                self.scroll(coordinates, direction, scroll_count, hold_keys.as_deref())
                    .await?;
                Ok(serde_json::json!({"success": true}))
            }

            ComputerAction::TypeKeys { keys, delay } => {
                if let Some(delay_ms) = delay {
                    self.press_keys_with_delay(&keys, delay_ms).await?;
                } else {
                    self.press_keys(&keys).await?;
                }
                Ok(serde_json::json!({"success": true}))
            }

            ComputerAction::PasteText { text } => {
                self.paste_text(&text).await?;
                Ok(serde_json::json!({"success": true}))
            }

            ComputerAction::PressKeys { keys, press } => {
                self.press_keys_with_type(&keys, press).await?;
                Ok(serde_json::json!({"success": true}))
            }

            ComputerAction::TypeText { text, delay, .. } => {
                if let Some(delay_ms) = delay {
                    self.type_text_with_delay(&text, delay_ms).await?;
                } else {
                    self.type_text(&text).await?;
                }
                Ok(serde_json::json!({"success": true}))
            }

            ComputerAction::Wait { duration } => {
                tokio::time::sleep(tokio::time::Duration::from_millis(duration)).await;
                Ok(serde_json::json!({"success": true}))
            }

            ComputerAction::Screenshot => {
                let image_data = self.take_screenshot().await?;
                Ok(serde_json::json!({
                    "success": true,
                    "image": image_data
                }))
            }

            ComputerAction::CursorPosition => {
                let position = self.get_cursor_position().await?;
                Ok(serde_json::json!({
                    "success": true,
                    "x": position.x,
                    "y": position.y
                }))
            }

            ComputerAction::Application { application } => {
                self.switch_application(application).await?;
                Ok(serde_json::json!({"success": true}))
            }

            ComputerAction::WriteFile { path, data } => {
                self.write_file(&path, &data).await?;
                Ok(serde_json::json!({
                    "success": true,
                    "message": "File written successfully"
                }))
            }

            ComputerAction::ReadFile { path } => {
                match self.read_file(&path).await {
                    Ok(data) => {
                        // Try to determine media type from file extension
                        let media_type = match path.split('.').next_back() {
                            Some("txt") => "text/plain",
                            Some("json") => "application/json",
                            Some("html") => "text/html",
                            Some("css") => "text/css",
                            Some("js") => "application/javascript",
                            Some("png") => "image/png",
                            Some("jpg") | Some("jpeg") => "image/jpeg",
                            Some("gif") => "image/gif",
                            Some("pdf") => "application/pdf",
                            _ => "application/octet-stream",
                        };

                        let file_name = path.split('/').next_back().unwrap_or("file");

                        Ok(serde_json::json!({
                            "success": true,
                            "data": data,
                            "mediaType": media_type,
                            "name": file_name,
                            "size": data.len()
                        }))
                    }
                    Err(e) => Ok(serde_json::json!({
                        "success": false,
                        "message": format!("Error reading file: {}", e)
                    })),
                }
            }
        }
    }

    /// Switch to a specific application
    async fn switch_application(&self, application: Application) -> Result<(), AutomationError> {
        self.applications.switch_to_application(application).await
    }
}

#[async_trait]
pub trait ComputerAutomation {
    // Screen operations
    async fn take_screenshot(&self) -> Result<String, AutomationError>;
    async fn get_cursor_position(&self) -> Result<Coordinates, AutomationError>;

    // Mouse operations
    async fn move_mouse(&self, coordinates: Coordinates) -> Result<(), AutomationError>;
    async fn click_mouse(
        &self,
        coordinates: Coordinates,
        button: Button,
    ) -> Result<(), AutomationError>;
    async fn click_mouse_with_options(
        &self,
        coordinates: Option<Coordinates>,
        button: Button,
        click_count: u32,
        hold_keys: Option<&[String]>,
    ) -> Result<(), AutomationError>;
    async fn press_mouse(
        &self,
        coordinates: Option<Coordinates>,
        button: Button,
        press_type: Press,
    ) -> Result<(), AutomationError>;
    async fn drag_mouse(
        &self,
        start: Coordinates,
        end: Coordinates,
        button: Button,
    ) -> Result<(), AutomationError>;
    async fn drag_mouse_path(
        &self,
        path: &[Coordinates],
        button: Button,
        hold_keys: Option<&[String]>,
    ) -> Result<(), AutomationError>;
    async fn trace_mouse_path(
        &self,
        path: &[Coordinates],
        hold_keys: Option<&[String]>,
    ) -> Result<(), AutomationError>;
    async fn scroll(
        &self,
        coordinates: Option<Coordinates>,
        direction: ScrollDirection,
        scroll_count: u32,
        hold_keys: Option<&[String]>,
    ) -> Result<(), AutomationError>;

    // Keyboard operations
    async fn type_text(&self, text: &str) -> Result<(), AutomationError>;
    async fn type_text_with_delay(&self, text: &str, delay_ms: u64) -> Result<(), AutomationError>;
    async fn press_keys(&self, keys: &[String]) -> Result<(), AutomationError>;
    async fn press_keys_with_delay(
        &self,
        keys: &[String],
        delay_ms: u64,
    ) -> Result<(), AutomationError>;
    async fn press_keys_with_type(
        &self,
        keys: &[String],
        press_type: Press,
    ) -> Result<(), AutomationError>;
    async fn paste_text(&self, text: &str) -> Result<(), AutomationError>;

    // File operations
    async fn read_file(&self, path: &str) -> Result<String, AutomationError>;
    async fn write_file(&self, path: &str, data: &str) -> Result<(), AutomationError>;
}

#[async_trait]
impl ComputerAutomation for AutomationService {
    // Screen operations
    async fn take_screenshot(&self) -> Result<String, AutomationError> {
        self.screen.take_screenshot().await
    }

    async fn get_cursor_position(&self) -> Result<Coordinates, AutomationError> {
        self.mouse.get_position().await
    }

    // Mouse operations
    async fn move_mouse(&self, coordinates: Coordinates) -> Result<(), AutomationError> {
        self.mouse.move_to(coordinates).await
    }

    async fn click_mouse(
        &self,
        coordinates: Coordinates,
        button: Button,
    ) -> Result<(), AutomationError> {
        self.mouse.click(coordinates, button).await
    }

    async fn click_mouse_with_options(
        &self,
        coordinates: Option<Coordinates>,
        button: Button,
        click_count: u32,
        hold_keys: Option<&[String]>,
    ) -> Result<(), AutomationError> {
        self.mouse
            .click_with_options(coordinates, button, click_count, hold_keys)
            .await
    }

    async fn press_mouse(
        &self,
        coordinates: Option<Coordinates>,
        button: Button,
        press_type: Press,
    ) -> Result<(), AutomationError> {
        self.mouse
            .press_with_type(coordinates, button, press_type)
            .await
    }

    async fn drag_mouse(
        &self,
        start: Coordinates,
        end: Coordinates,
        button: Button,
    ) -> Result<(), AutomationError> {
        self.mouse.drag(start, end, button).await
    }

    async fn drag_mouse_path(
        &self,
        path: &[Coordinates],
        button: Button,
        hold_keys: Option<&[String]>,
    ) -> Result<(), AutomationError> {
        self.mouse.drag_path(path, button, hold_keys).await
    }

    async fn trace_mouse_path(
        &self,
        path: &[Coordinates],
        hold_keys: Option<&[String]>,
    ) -> Result<(), AutomationError> {
        self.mouse.trace_path(path, hold_keys).await
    }

    async fn scroll(
        &self,
        coordinates: Option<Coordinates>,
        direction: ScrollDirection,
        scroll_count: u32,
        hold_keys: Option<&[String]>,
    ) -> Result<(), AutomationError> {
        self.mouse
            .scroll_direction(coordinates, direction, scroll_count, hold_keys)
            .await
    }

    // Keyboard operations
    async fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        self.keyboard.type_text(text).await
    }

    async fn type_text_with_delay(&self, text: &str, delay_ms: u64) -> Result<(), AutomationError> {
        self.keyboard.type_text_with_delay(text, delay_ms).await
    }

    async fn press_keys(&self, keys: &[String]) -> Result<(), AutomationError> {
        self.keyboard.press_keys(keys).await
    }

    async fn press_keys_with_delay(
        &self,
        keys: &[String],
        delay_ms: u64,
    ) -> Result<(), AutomationError> {
        self.keyboard.press_keys_with_delay(keys, delay_ms).await
    }

    async fn press_keys_with_type(
        &self,
        keys: &[String],
        press_type: Press,
    ) -> Result<(), AutomationError> {
        for key in keys {
            self.keyboard.press_key_with_type(key, press_type).await?;
        }
        Ok(())
    }

    async fn paste_text(&self, text: &str) -> Result<(), AutomationError> {
        self.keyboard.paste_text(text).await
    }

    // File operations
    async fn read_file(&self, path: &str) -> Result<String, AutomationError> {
        self.files.read_file(path).await
    }

    async fn write_file(&self, path: &str, data: &str) -> Result<(), AutomationError> {
        self.files.write_file(path, data).await
    }
}
