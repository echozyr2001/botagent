use bytebot_shared_rs::types::computer_action::{Button, Coordinates};
use enigo::{Enigo, Mouse, Settings};
use tracing::{debug, error};

use crate::error::AutomationError;

#[derive(Debug, Clone)]
pub struct MouseService;

impl MouseService {
    pub fn new() -> Result<Self, AutomationError> {
        // Test that we can create an Enigo instance
        let _test_enigo = Enigo::new(&Settings::default()).map_err(|e| {
            AutomationError::InputFailed(format!("Failed to initialize mouse control: {e}"))
        })?;

        Ok(Self)
    }

    fn create_enigo(&self) -> Result<Enigo, AutomationError> {
        Enigo::new(&Settings::default()).map_err(|e| {
            AutomationError::InputFailed(format!("Failed to create Enigo instance: {e}"))
        })
    }

    /// Move mouse to specified coordinates
    pub async fn move_to(&self, coordinates: Coordinates) -> Result<(), AutomationError> {
        debug!("Moving mouse to ({}, {})", coordinates.x, coordinates.y);

        self.validate_coordinates(&coordinates)?;

        let mut enigo = self.create_enigo()?;

        enigo
            .move_mouse(coordinates.x, coordinates.y, enigo::Coordinate::Abs)
            .map_err(|e| {
                error!("Failed to move mouse: {}", e);
                AutomationError::InputFailed(format!("Mouse move failed: {e}"))
            })?;

        Ok(())
    }

    /// Click mouse at specified coordinates with given button
    pub async fn click(
        &self,
        coordinates: Coordinates,
        button: Button,
    ) -> Result<(), AutomationError> {
        debug!(
            "Clicking mouse at ({}, {}) with {:?} button",
            coordinates.x, coordinates.y, button
        );

        self.validate_coordinates(&coordinates)?;

        // Move to coordinates first
        self.move_to(coordinates).await?;

        // Small delay to ensure mouse has moved
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let mut enigo = self.create_enigo()?;

        let enigo_button = self.convert_button(button)?;

        enigo
            .button(enigo_button, enigo::Direction::Click)
            .map_err(|e| {
                error!("Failed to click mouse: {}", e);
                AutomationError::InputFailed(format!("Mouse click failed: {e}"))
            })?;

        Ok(())
    }

    /// Press and hold mouse button at coordinates
    pub async fn press(
        &self,
        coordinates: Coordinates,
        button: Button,
    ) -> Result<(), AutomationError> {
        debug!(
            "Pressing mouse button {:?} at ({}, {})",
            button, coordinates.x, coordinates.y
        );

        self.validate_coordinates(&coordinates)?;
        self.move_to(coordinates).await?;

        let mut enigo = self.create_enigo()?;

        let enigo_button = self.convert_button(button)?;

        enigo
            .button(enigo_button, enigo::Direction::Press)
            .map_err(|e| {
                error!("Failed to press mouse button: {}", e);
                AutomationError::InputFailed(format!("Mouse press failed: {e}"))
            })?;

        Ok(())
    }

    /// Release mouse button
    pub async fn release(&self, button: Button) -> Result<(), AutomationError> {
        debug!("Releasing mouse button {:?}", button);

        let mut enigo = self.create_enigo()?;

        let enigo_button = self.convert_button(button)?;

        enigo
            .button(enigo_button, enigo::Direction::Release)
            .map_err(|e| {
                error!("Failed to release mouse button: {}", e);
                AutomationError::InputFailed(format!("Mouse release failed: {e}"))
            })?;

        Ok(())
    }

    /// Drag from start coordinates to end coordinates
    pub async fn drag(
        &self,
        start: Coordinates,
        end: Coordinates,
        button: Button,
    ) -> Result<(), AutomationError> {
        debug!(
            "Dragging from ({}, {}) to ({}, {}) with {:?} button",
            start.x, start.y, end.x, end.y, button
        );

        self.validate_coordinates(&start)?;
        self.validate_coordinates(&end)?;

        // Move to start position and press button
        self.press(start, button).await?;

        // Small delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Move to end position while holding button
        self.move_to(end).await?;

        // Small delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Release button
        self.release(button).await?;

        Ok(())
    }

    /// Scroll at coordinates
    pub async fn scroll(
        &self,
        coordinates: Coordinates,
        delta_x: i32,
        delta_y: i32,
    ) -> Result<(), AutomationError> {
        debug!(
            "Scrolling at ({}, {}) with delta ({}, {})",
            coordinates.x, coordinates.y, delta_x, delta_y
        );

        self.validate_coordinates(&coordinates)?;
        self.move_to(coordinates).await?;

        let mut enigo = self.create_enigo()?;

        // Scroll vertically if delta_y is non-zero
        if delta_y != 0 {
            enigo.scroll(delta_y, enigo::Axis::Vertical).map_err(|e| {
                error!("Failed to scroll vertically: {}", e);
                AutomationError::InputFailed(format!("Vertical scroll failed: {e}"))
            })?;
        }

        // Scroll horizontally if delta_x is non-zero
        if delta_x != 0 {
            enigo
                .scroll(delta_x, enigo::Axis::Horizontal)
                .map_err(|e| {
                    error!("Failed to scroll horizontally: {}", e);
                    AutomationError::InputFailed(format!("Horizontal scroll failed: {e}"))
                })?;
        }

        Ok(())
    }

    /// Get current mouse position
    pub async fn get_position(&self) -> Result<Coordinates, AutomationError> {
        let enigo = self.create_enigo()?;

        let (x, y) = enigo.location().map_err(|e| {
            error!("Failed to get mouse position: {}", e);
            AutomationError::InputFailed(format!("Failed to get mouse position: {e}"))
        })?;

        Ok(Coordinates { x, y })
    }

    fn validate_coordinates(&self, coordinates: &Coordinates) -> Result<(), AutomationError> {
        if coordinates.x < 0 || coordinates.y < 0 {
            return Err(AutomationError::InvalidCoordinates {
                x: coordinates.x,
                y: coordinates.y,
            });
        }

        // Additional validation could be added here for screen bounds
        Ok(())
    }

    fn convert_button(&self, button: Button) -> Result<enigo::Button, AutomationError> {
        match button {
            Button::Left => Ok(enigo::Button::Left),
            Button::Right => Ok(enigo::Button::Right),
            Button::Middle => Ok(enigo::Button::Middle),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mouse_service_creation() {
        let result = MouseService::new();
        assert!(
            result.is_ok(),
            "Mouse service should initialize successfully"
        );
    }

    #[tokio::test]
    async fn test_validate_coordinates() {
        let service = MouseService::new().expect("Failed to create mouse service");

        // Valid coordinates
        let valid_coords = Coordinates { x: 100, y: 100 };
        assert!(service.validate_coordinates(&valid_coords).is_ok());

        // Invalid coordinates
        let invalid_coords = Coordinates { x: -1, y: 100 };
        assert!(service.validate_coordinates(&invalid_coords).is_err());
    }

    #[tokio::test]
    async fn test_convert_button() {
        let service = MouseService::new().expect("Failed to create mouse service");

        assert!(matches!(
            service.convert_button(Button::Left),
            Ok(enigo::Button::Left)
        ));
        assert!(matches!(
            service.convert_button(Button::Right),
            Ok(enigo::Button::Right)
        ));
        assert!(matches!(
            service.convert_button(Button::Middle),
            Ok(enigo::Button::Middle)
        ));
    }
}
