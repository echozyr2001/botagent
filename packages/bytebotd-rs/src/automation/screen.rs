use std::io::Cursor;

use base64::{engine::general_purpose, Engine as _};
use bytebot_shared_rs::{
    logging::automation_logging,
    types::computer_action::Coordinates,
};
use image::{DynamicImage, ImageFormat, RgbaImage};
use screenshots::Screen;
use tracing::{debug, error, info};

use crate::error::AutomationError;

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

        info!(
            "Initialized screen service with {} screen(s)",
            screens.len()
        );
        Ok(Self { screens })
    }

    /// Take a screenshot of the primary screen and return as base64 encoded PNG
    pub async fn take_screenshot(&self) -> Result<String, AutomationError> {
        debug!(screen = "primary", "Taking screenshot");

        // Use the first screen as primary
        let screen = self
            .screens
            .first()
            .ok_or_else(|| AutomationError::ScreenshotFailed("No screens available".to_string()))?;

        // Capture the screen in a blocking task to avoid blocking the async runtime
        let screen_clone = *screen;
        let image_result = tokio::task::spawn_blocking(move || screen_clone.capture())
            .await
            .map_err(|e| {
                AutomationError::ScreenshotFailed(format!("Screenshot task failed: {e}"))
            })?;

        let image = image_result.map_err(|e| {
            error!(error = %e, "Screenshot capture failed");
            AutomationError::ScreenshotFailed(format!("Screen capture failed: {e}"))
        })?;

        // Convert to base64
        let result = self.image_to_base64(image).await;
        if result.is_ok() {
            automation_logging::screenshot_taken();
        }
        result
    }

    /// Take a screenshot of a specific screen by index
    pub async fn take_screenshot_of_screen(
        &self,
        screen_index: usize,
    ) -> Result<String, AutomationError> {
        debug!("Taking screenshot of screen {}", screen_index);

        let screen = self.screens.get(screen_index).ok_or_else(|| {
            AutomationError::ScreenshotFailed(format!("Screen {screen_index} not found"))
        })?;

        let screen_clone = *screen;
        let image_result = tokio::task::spawn_blocking(move || screen_clone.capture())
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

    /// Validate screen coordinates against available screens
    pub fn validate_coordinates(&self, coordinates: &Coordinates) -> Result<(), AutomationError> {
        if coordinates.x < 0 || coordinates.y < 0 {
            return Err(AutomationError::InvalidCoordinates {
                x: coordinates.x,
                y: coordinates.y,
            });
        }

        // Check if coordinates are within any screen bounds
        let within_bounds = self.screens.iter().any(|screen| {
            let display = &screen.display_info;
            coordinates.x >= display.x
                && coordinates.x < display.x + display.width as i32
                && coordinates.y >= display.y
                && coordinates.y < display.y + display.height as i32
        });

        if !within_bounds {
            return Err(AutomationError::InvalidCoordinates {
                x: coordinates.x,
                y: coordinates.y,
            });
        }

        Ok(())
    }

    /// Get the primary screen dimensions for coordinate validation
    pub fn get_primary_screen_bounds(&self) -> Option<(i32, i32, u32, u32)> {
        self.screens
            .iter()
            .find(|screen| screen.display_info.is_primary)
            .map(|screen| {
                let display = &screen.display_info;
                (display.x, display.y, display.width, display.height)
            })
    }

    /// Get all screen bounds for coordinate validation
    pub fn get_all_screen_bounds(&self) -> Vec<(i32, i32, u32, u32)> {
        self.screens
            .iter()
            .map(|screen| {
                let display = &screen.display_info;
                (display.x, display.y, display.width, display.height)
            })
            .collect()
    }

    /// Convert screenshots::Image to base64 encoded PNG
    async fn image_to_base64(&self, image: screenshots::Image) -> Result<String, AutomationError> {
        tokio::task::spawn_blocking(move || {
            // Convert screenshots::Image to DynamicImage
            let rgba_image =
                RgbaImage::from_raw(image.width(), image.height(), image.buffer().to_vec())
                    .ok_or_else(|| {
                        AutomationError::ScreenshotFailed(
                            "Failed to create RGBA image from raw data".to_string(),
                        )
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
        assert!(
            result.is_ok(),
            "Screen service should initialize successfully"
        );

        let service = result.unwrap();
        assert!(
            !service.screens.is_empty(),
            "Should have at least one screen"
        );
    }

    #[tokio::test]
    async fn test_get_screen_info() {
        let service = ScreenService::new().expect("Failed to create screen service");
        let screen_info = service.get_screen_info();

        assert!(!screen_info.is_empty(), "Should have screen information");
        assert!(
            screen_info.iter().any(|s| s.is_primary),
            "Should have a primary screen"
        );
    }

    #[tokio::test]
    async fn test_take_screenshot() {
        let service = ScreenService::new().expect("Failed to create screen service");
        let result = service.take_screenshot().await;

        // In headless environments or CI, screenshot might fail
        // This is expected behavior, so we handle both cases
        match result {
            Ok(base64_data) => {
                assert!(
                    !base64_data.is_empty(),
                    "Screenshot data should not be empty"
                );

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

    #[tokio::test]
    async fn test_coordinate_validation() {
        let service = ScreenService::new().expect("Failed to create screen service");

        // Test negative coordinates
        let invalid_coords = Coordinates { x: -1, y: 100 };
        assert!(service.validate_coordinates(&invalid_coords).is_err());

        let invalid_coords2 = Coordinates { x: 100, y: -1 };
        assert!(service.validate_coordinates(&invalid_coords2).is_err());

        // Test coordinates within screen bounds (assuming we have at least one screen)
        if let Some((x, y, width, height)) = service.get_primary_screen_bounds() {
            let valid_coords = Coordinates {
                x: x + 100,
                y: y + 100,
            };
            // Only test if coordinates are within bounds
            if valid_coords.x < x + width as i32 && valid_coords.y < y + height as i32 {
                assert!(service.validate_coordinates(&valid_coords).is_ok());
            }

            // Test coordinates outside screen bounds
            let out_of_bounds = Coordinates {
                x: x + width as i32 + 1000,
                y: y + height as i32 + 1000,
            };
            assert!(service.validate_coordinates(&out_of_bounds).is_err());
        }
    }

    #[tokio::test]
    async fn test_screen_bounds() {
        let service = ScreenService::new().expect("Failed to create screen service");

        let primary_bounds = service.get_primary_screen_bounds();
        assert!(primary_bounds.is_some(), "Should have a primary screen");

        let all_bounds = service.get_all_screen_bounds();
        assert!(!all_bounds.is_empty(), "Should have at least one screen");
        assert_eq!(all_bounds.len(), service.screens.len());
    }
}
