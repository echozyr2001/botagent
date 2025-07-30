use bytebot_shared_rs::types::computer_action::{Button, Coordinates, Press, ScrollDirection};
use enigo::{Enigo, Mouse, Settings};
use tracing::{debug, error, warn};

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

    /// Drag along a path of coordinates
    pub async fn drag_path(
        &self,
        path: &[Coordinates],
        button: Button,
        hold_keys: Option<&[String]>,
    ) -> Result<(), AutomationError> {
        if path.is_empty() {
            return Err(AutomationError::Validation("Path cannot be empty".to_string()));
        }

        debug!(
            "Dragging along path with {} points using {:?} button",
            path.len(),
            button
        );

        // Validate all coordinates in the path
        for coord in path {
            self.validate_coordinates(coord)?;
        }

        // TODO: Handle hold_keys if provided
        if hold_keys.is_some() {
            warn!("Hold keys during drag not yet implemented");
        }

        let start = path[0];
        
        // Move to start position and press button
        self.press(start, button).await?;

        // Small delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Move through each point in the path
        for coord in &path[1..] {
            self.move_to(*coord).await?;
            // Small delay between movements for smooth dragging
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Release button at the end
        self.release(button).await?;

        Ok(())
    }

    /// Trace mouse along a path without clicking
    pub async fn trace_path(
        &self,
        path: &[Coordinates],
        hold_keys: Option<&[String]>,
    ) -> Result<(), AutomationError> {
        if path.is_empty() {
            return Err(AutomationError::Validation("Path cannot be empty".to_string()));
        }

        debug!("Tracing mouse along path with {} points", path.len());

        // Validate all coordinates in the path
        for coord in path {
            self.validate_coordinates(coord)?;
        }

        // TODO: Handle hold_keys if provided
        if hold_keys.is_some() {
            warn!("Hold keys during trace not yet implemented");
        }

        // Move through each point in the path
        for coord in path {
            self.move_to(*coord).await?;
            // Small delay between movements for smooth tracing
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        }

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

    /// Scroll using direction and count
    pub async fn scroll_direction(
        &self,
        coordinates: Option<Coordinates>,
        direction: ScrollDirection,
        scroll_count: u32,
        hold_keys: Option<&[String]>,
    ) -> Result<(), AutomationError> {
        if scroll_count == 0 {
            return Err(AutomationError::Validation("Scroll count must be greater than 0".to_string()));
        }

        let coords = coordinates.unwrap_or({
            // If no coordinates provided, use current mouse position
            // For now, use a default position
            Coordinates { x: 500, y: 500 }
        });

        debug!(
            "Scrolling {:?} {} times at ({}, {})",
            direction, scroll_count, coords.x, coords.y
        );

        self.validate_coordinates(&coords)?;

        // TODO: Handle hold_keys if provided
        if hold_keys.is_some() {
            warn!("Hold keys during scroll not yet implemented");
        }

        // Move to coordinates first
        self.move_to(coords).await?;

        // Convert direction to scroll deltas
        let (delta_x, delta_y) = match direction {
            ScrollDirection::Up => (0, 3),
            ScrollDirection::Down => (0, -3),
            ScrollDirection::Left => (-3, 0),
            ScrollDirection::Right => (3, 0),
        };

        // Perform scroll multiple times
        for _ in 0..scroll_count {
            {
                let mut enigo = self.create_enigo()?;
                
                if delta_y != 0 {
                    enigo.scroll(delta_y, enigo::Axis::Vertical).map_err(|e| {
                        error!("Failed to scroll vertically: {}", e);
                        AutomationError::InputFailed(format!("Vertical scroll failed: {e}"))
                    })?;
                }

                if delta_x != 0 {
                    enigo.scroll(delta_x, enigo::Axis::Horizontal).map_err(|e| {
                        error!("Failed to scroll horizontally: {}", e);
                        AutomationError::InputFailed(format!("Horizontal scroll failed: {e}"))
                    })?;
                }
            } // enigo is dropped here

            // Small delay between scrolls
            if scroll_count > 1 {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }

        Ok(())
    }

    /// Press mouse button with specific press type (up/down)
    pub async fn press_with_type(
        &self,
        coordinates: Option<Coordinates>,
        button: Button,
        press_type: Press,
    ) -> Result<(), AutomationError> {
        debug!(
            "Pressing mouse button {:?} with type {:?} at coordinates: {:?}",
            button, press_type, coordinates
        );

        if let Some(coords) = coordinates {
            self.validate_coordinates(&coords)?;
            self.move_to(coords).await?;
        }

        let mut enigo = self.create_enigo()?;
        let enigo_button = self.convert_button(button)?;

        let direction = match press_type {
            Press::Down => enigo::Direction::Press,
            Press::Up => enigo::Direction::Release,
        };

        enigo.button(enigo_button, direction).map_err(|e| {
            error!("Failed to press mouse button: {}", e);
            AutomationError::InputFailed(format!("Mouse button press failed: {e}"))
        })?;

        Ok(())
    }

    /// Click mouse with multiple clicks and optional hold keys
    pub async fn click_with_options(
        &self,
        coordinates: Option<Coordinates>,
        button: Button,
        click_count: u32,
        hold_keys: Option<&[String]>,
    ) -> Result<(), AutomationError> {
        if click_count == 0 {
            return Err(AutomationError::Validation("Click count must be greater than 0".to_string()));
        }

        let coords = coordinates.unwrap_or({
            // If no coordinates provided, click at current position
            // For now, we'll get the current position or use a default
            Coordinates { x: 0, y: 0 }
        });

        debug!(
            "Clicking mouse at ({}, {}) with {:?} button, {} times",
            coords.x, coords.y, button, click_count
        );

        if coordinates.is_some() {
            self.validate_coordinates(&coords)?;
            self.move_to(coords).await?;
        }

        // TODO: Handle hold_keys if provided
        if hold_keys.is_some() {
            warn!("Hold keys during click not yet implemented");
        }

        let enigo_button = self.convert_button(button)?;

        // Perform multiple clicks
        for i in 0..click_count {
            {
                let mut enigo = self.create_enigo()?;
                enigo.button(enigo_button, enigo::Direction::Click).map_err(|e| {
                    error!("Failed to click mouse: {}", e);
                    AutomationError::InputFailed(format!("Mouse click failed: {e}"))
                })?;
            } // enigo is dropped here

            // Small delay between multiple clicks
            if i < click_count - 1 {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
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

    #[tokio::test]
    async fn test_drag_path_validation() {
        let service = MouseService::new().expect("Failed to create mouse service");

        // Test empty path
        let empty_path = vec![];
        let result = service.drag_path(&empty_path, Button::Left, None).await;
        assert!(result.is_err());

        // Test invalid coordinates in path
        let invalid_path = vec![
            Coordinates { x: 100, y: 100 },
            Coordinates { x: -1, y: 200 },
        ];
        let result = service.drag_path(&invalid_path, Button::Left, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_trace_path_validation() {
        let service = MouseService::new().expect("Failed to create mouse service");

        // Test empty path
        let empty_path = vec![];
        let result = service.trace_path(&empty_path, None).await;
        assert!(result.is_err());

        // Test invalid coordinates in path
        let invalid_path = vec![
            Coordinates { x: 100, y: 100 },
            Coordinates { x: 200, y: -1 },
        ];
        let result = service.trace_path(&invalid_path, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_scroll_direction_validation() {
        let service = MouseService::new().expect("Failed to create mouse service");

        // Test zero scroll count
        let result = service.scroll_direction(
            Some(Coordinates { x: 100, y: 100 }),
            ScrollDirection::Up,
            0,
            None,
        ).await;
        assert!(result.is_err());

        // Test invalid coordinates
        let result = service.scroll_direction(
            Some(Coordinates { x: -1, y: 100 }),
            ScrollDirection::Up,
            1,
            None,
        ).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_click_with_options_validation() {
        let service = MouseService::new().expect("Failed to create mouse service");

        // Test zero click count
        let result = service.click_with_options(
            Some(Coordinates { x: 100, y: 100 }),
            Button::Left,
            0,
            None,
        ).await;
        assert!(result.is_err());

        // Test invalid coordinates
        let result = service.click_with_options(
            Some(Coordinates { x: -1, y: 100 }),
            Button::Left,
            1,
            None,
        ).await;
        assert!(result.is_err());
    }
}
