pub mod screen;
pub mod mouse;
pub mod keyboard;
pub mod applications;
pub mod files;

use crate::error::AutomationError;
use async_trait::async_trait;
use bytebot_shared_rs::types::computer_action::{Coordinates, Button};

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
}

#[async_trait]
pub trait ComputerAutomation {
    async fn take_screenshot(&self) -> Result<String, AutomationError>;
    async fn move_mouse(&self, coordinates: Coordinates) -> Result<(), AutomationError>;
    async fn click_mouse(&self, coordinates: Coordinates, button: Button) -> Result<(), AutomationError>;
    async fn type_text(&self, text: &str) -> Result<(), AutomationError>;
    async fn press_keys(&self, keys: &[String]) -> Result<(), AutomationError>;
    async fn read_file(&self, path: &str) -> Result<String, AutomationError>;
    async fn write_file(&self, path: &str, data: &str) -> Result<(), AutomationError>;
}

#[async_trait]
impl ComputerAutomation for AutomationService {
    async fn take_screenshot(&self) -> Result<String, AutomationError> {
        self.screen.take_screenshot().await
    }

    async fn move_mouse(&self, coordinates: Coordinates) -> Result<(), AutomationError> {
        self.mouse.move_to(coordinates).await
    }

    async fn click_mouse(&self, coordinates: Coordinates, button: Button) -> Result<(), AutomationError> {
        self.mouse.click(coordinates, button).await
    }

    async fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        self.keyboard.type_text(text).await
    }

    async fn press_keys(&self, keys: &[String]) -> Result<(), AutomationError> {
        self.keyboard.press_keys(keys).await
    }

    async fn read_file(&self, path: &str) -> Result<String, AutomationError> {
        self.files.read_file(path).await
    }

    async fn write_file(&self, path: &str, data: &str) -> Result<(), AutomationError> {
        self.files.write_file(path, data).await
    }
}