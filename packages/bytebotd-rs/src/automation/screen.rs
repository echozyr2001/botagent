use crate::error::AutomationError;
use base64::{engine::general_purpose, Engine as _};
use image::{ImageFormat, RgbaImage, DynamicImage};
use screenshots::Screen;
use std::io::Cursor;
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
pub struct ScreenService {
    screens: Vec<Screen>,
}

impl ScreenService {
    pub fn new() -> Result<Self, AutomationError> {
        let screens = Screen::all().map_err(|e| {
            AutomationError::ScreenshotFailed(format!("Failed to enumerate screens: {e}"))
        })?;

        if screens.is_empty() {
            return Err(AutomationError::ScreenshotFailed(
                "No screens found".to_string(),
            ));
        }

        info!("Initialized screen service with {} screen(s)", screens.len());
        Ok(Self { screens })
    }

    /// Take a screenshot of the primary screen and return as base64 encoded PNG
    pub async fn take_screenshot(&self) -> Result<String, AutomationError> {
        debug!("Taking screenshot of primary screen");

        // Use the first screen as primary
        let screen = self.screens.first().ok_or_else(|| {
            AutomationError::ScreenshotFailed("No screens available".to_string())
        })?;

        // Capture the screen in a blocking task to avoid blocking the async runtime
        let screen_clone = screen.clone();
        let image_result = tokio::task::spawn_blocking(move || {
            screen_clone.capture()
        })
        .await
        .map_err(|e| {
            AutomationError::ScreenshotFailed(format!("Screenshot task failed: {e}"))
        })?;

        let image = image_result.map_err(|e| {
            error!("Screenshot capture failed: {}", e);
            AutomationError::ScreenshotFailed(format!("Screen capture failed: {e}"))
        })?;

        // Convert to base64
        self.image_to_base64(image).await
    }

    /// Take a screenshot of a specific screen by index
    pub async fn take_screenshot_of_screen(&self, screen_index: usize) -> Result<String, AutomationError> {
        debug!("Taking screenshot of screen {}", screen_index);

        let screen = self.screens.get(screen_index).ok_or_else(|| {
            AutomationError::ScreenshotFailed(format!("Screen {screen_index} not found"))
        })?;

        let screen_clone = screen.clone();
        let image_result = tokio::task::spawn_blocking(move || {
            screen_clone.capture()
        })
        .await
        .map_err(|e| {
            AutomationError::ScreenshotFailed(format!("Screenshot task failed: {e}"))
        })?;

        let image = image_result.map_err(|e| {
            error!("Screenshot capture failed: {}", e);
            AutomationError::ScreenshotFailed(format!("Screen capture failed: {e}"))
        })?;

        self.image_to_base64(image).await
    }

    /// Get information about all available screens
    pub fn get_screen_info(&self) -> Vec<ScreenInfo> {
        self.screens
            .iter()
            .enumerate()
            .map(|(index, screen)| ScreenInfo {
                index,
                width: screen.display_info.width,
                height: screen.display_info.height,
                x: screen.display_info.x,
                y: screen.display_info.y,
                is_primary: screen.display_info.is_primary,
            })
            .collect()
    }

    /// Convert screenshots::Image to base64 encoded PNG
    async fn image_to_base64(&self, image: screenshots::Image) -> Result<String, AutomationError> {
        tokio::task::spawn_blocking(move || {
            // Convert screenshots::Image to DynamicImage
            let rgba_image = RgbaImage::from_raw(
                image.width(),
                image.height(),
                image.buffer().to_vec(),
            ).ok_or_else(|| {
                AutomationError::ScreenshotFailed("Failed to create RGBA image from raw data".to_string())
            })?;

            let dynamic_image = DynamicImage::ImageRgba8(rgba_image);
            
            let mut buffer = Cursor::new(Vec::new());
            
            // Convert to PNG format
            dynamic_image
                .write_to(&mut buffer, ImageFormat::Png)
                .map_err(|e| {
                    AutomationError::ScreenshotFailed(format!("Failed to encode image as PNG: {e}"))
                })?;

            // Encode as base64
            let base64_string = general_purpose::STANDARD.encode(buffer.into_inner());
            Ok(base64_string)
        })
        .await
        .map_err(|e| {
            AutomationError::ScreenshotFailed(format!("Image encoding task failed: {e}"))
        })?
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScreenInfo {
    pub index: usize,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub is_primary: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_screen_service_creation() {
        let result = ScreenService::new();
        assert!(result.is_ok(), "Screen service should initialize successfully");
        
        let service = result.unwrap();
        assert!(!service.screens.is_empty(), "Should have at least one screen");
    }

    #[tokio::test]
    async fn test_get_screen_info() {
        let service = ScreenService::new().expect("Failed to create screen service");
        let screen_info = service.get_screen_info();
        
        assert!(!screen_info.is_empty(), "Should have screen information");
        assert!(screen_info.iter().any(|s| s.is_primary), "Should have a primary screen");
    }

    #[tokio::test]
    async fn test_take_screenshot() {
        let service = ScreenService::new().expect("Failed to create screen service");
        let result = service.take_screenshot().await;
        
        // In headless environments or CI, screenshot might fail
        // This is expected behavior, so we handle both cases
        match result {
            Ok(base64_data) => {
                assert!(!base64_data.is_empty(), "Screenshot data should not be empty");
                
                // Verify it's valid base64
                let decoded = general_purpose::STANDARD.decode(&base64_data);
                assert!(decoded.is_ok(), "Screenshot should be valid base64");
            }
            Err(e) => {
                // In headless/CI environments, this is expected
                println!("Screenshot failed (expected in headless environment): {e}");
                // We don't fail the test, just log the expected failure
            }
        }
    }
}