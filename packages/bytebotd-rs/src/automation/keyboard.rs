use bytebot_shared_rs::types::computer_action::Press;
use enigo::{Enigo, Keyboard, Settings};
use tracing::{debug, error};

use crate::error::AutomationError;

#[derive(Debug, Clone)]
pub struct KeyboardService;

impl KeyboardService {
    pub fn new() -> Result<Self, AutomationError> {
        // Test that we can create an Enigo instance
        let _test_enigo = Enigo::new(&Settings::default()).map_err(|e| {
            AutomationError::InputFailed(format!("Failed to initialize keyboard control: {e}"))
        })?;

        Ok(Self)
    }

    fn create_enigo(&self) -> Result<Enigo, AutomationError> {
        Enigo::new(&Settings::default()).map_err(|e| {
            AutomationError::InputFailed(format!("Failed to create Enigo instance: {e}"))
        })
    }

    /// Type text with optional delay between characters
    pub async fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        debug!("Typing text: {}", text);

        if text.is_empty() {
            return Err(AutomationError::Validation("Text cannot be empty".to_string()));
        }

        let mut enigo = self.create_enigo()?;

        enigo.text(text).map_err(|e| {
            error!("Failed to type text: {}", e);
            AutomationError::InputFailed(format!("Text input failed: {e}"))
        })?;

        Ok(())
    }

    /// Type text with specified delay between characters
    pub async fn type_text_with_delay(
        &self,
        text: &str,
        delay_ms: u64,
    ) -> Result<(), AutomationError> {
        debug!("Typing text with {}ms delay: {}", delay_ms, text);

        if text.is_empty() {
            return Err(AutomationError::Validation("Text cannot be empty".to_string()));
        }

        // Validate delay
        if delay_ms > 60000 {
            return Err(AutomationError::Validation("Delay cannot exceed 60 seconds".to_string()));
        }

        for ch in text.chars() {
            // Type each character individually
            {
                let mut enigo = self.create_enigo()?;
                enigo.text(&ch.to_string()).map_err(|e| {
                    error!("Failed to type character '{}': {}", ch, e);
                    AutomationError::InputFailed(format!("Character input failed for '{ch}': {e}"))
                })?;
            } // enigo is dropped here

            if delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            }
        }

        Ok(())
    }

    /// Press specific keys
    pub async fn press_keys(&self, keys: &[String]) -> Result<(), AutomationError> {
        debug!("Pressing keys: {:?}", keys);

        if keys.is_empty() {
            return Err(AutomationError::Validation("Keys array cannot be empty".to_string()));
        }

        // Validate all keys first
        for key in keys {
            if key.is_empty() {
                return Err(AutomationError::Validation("Key cannot be empty".to_string()));
            }
            // Validate that the key can be converted
            self.convert_key(key)?;
        }

        for key in keys {
            {
                let mut enigo = self.create_enigo()?;
                let enigo_key = self.convert_key(key)?;
                enigo.key(enigo_key, enigo::Direction::Click).map_err(|e| {
                    error!("Failed to press key '{}': {}", key, e);
                    AutomationError::InputFailed(format!("Key press failed for '{key}': {e}"))
                })?;
            } // enigo is dropped here

            // Small delay between key presses
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        Ok(())
    }

    /// Press keys with delay between each key
    pub async fn press_keys_with_delay(
        &self,
        keys: &[String],
        delay_ms: u64,
    ) -> Result<(), AutomationError> {
        debug!("Pressing keys with {}ms delay: {:?}", delay_ms, keys);

        if keys.is_empty() {
            return Err(AutomationError::Validation("Keys array cannot be empty".to_string()));
        }

        // Validate delay
        if delay_ms > 60000 {
            return Err(AutomationError::Validation("Delay cannot exceed 60 seconds".to_string()));
        }

        // Validate all keys first
        for key in keys {
            if key.is_empty() {
                return Err(AutomationError::Validation("Key cannot be empty".to_string()));
            }
            self.convert_key(key)?;
        }

        for key in keys {
            {
                let mut enigo = self.create_enigo()?;
                let enigo_key = self.convert_key(key)?;
                enigo.key(enigo_key, enigo::Direction::Click).map_err(|e| {
                    error!("Failed to press key '{}': {}", key, e);
                    AutomationError::InputFailed(format!("Key press failed for '{key}': {e}"))
                })?;
            }

            // Apply specified delay
            if delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            }
        }

        Ok(())
    }

    /// Press key with specified press type (up, down)
    pub async fn press_key_with_type(
        &self,
        key: &str,
        press_type: Press,
    ) -> Result<(), AutomationError> {
        debug!("Pressing key '{}' with type {:?}", key, press_type);

        let mut enigo = self.create_enigo()?;

        let enigo_key = self.convert_key(key)?;
        let direction = match press_type {
            Press::Up => enigo::Direction::Release,
            Press::Down => enigo::Direction::Press,
        };

        enigo.key(enigo_key, direction).map_err(|e| {
            error!("Failed to press key '{}': {}", key, e);
            AutomationError::InputFailed(format!("Key press failed for '{key}': {e}"))
        })?;

        Ok(())
    }

    /// Press key combination (e.g., Ctrl+C)
    pub async fn press_key_combination(&self, keys: &[String]) -> Result<(), AutomationError> {
        debug!("Pressing key combination: {:?}", keys);

        if keys.is_empty() {
            return Ok(());
        }

        let mut enigo = self.create_enigo()?;

        // Press all keys down
        let mut enigo_keys = Vec::new();
        for key in keys {
            let enigo_key = self.convert_key(key)?;
            enigo_keys.push(enigo_key);
            enigo.key(enigo_key, enigo::Direction::Press).map_err(|e| {
                error!("Failed to press key '{}': {}", key, e);
                AutomationError::InputFailed(format!("Key press failed for '{key}': {e}"))
            })?;
        }

        // Small delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Release all keys in reverse order
        for enigo_key in enigo_keys.iter().rev() {
            enigo
                .key(*enigo_key, enigo::Direction::Release)
                .map_err(|e| {
                    error!("Failed to release key: {}", e);
                    AutomationError::InputFailed(format!("Key release failed: {e}"))
                })?;
        }

        Ok(())
    }

    /// Paste text from clipboard
    pub async fn paste(&self) -> Result<(), AutomationError> {
        debug!("Pasting from clipboard");
        self.press_key_combination(&["ctrl".to_string(), "v".to_string()])
            .await
    }

    /// Paste specific text (simulates typing the text directly)
    pub async fn paste_text(&self, text: &str) -> Result<(), AutomationError> {
        debug!("Pasting text directly: {}", text);
        
        if text.is_empty() {
            return Err(AutomationError::Validation("Text cannot be empty".to_string()));
        }

        // For paste_text, we directly type the text instead of using clipboard
        self.type_text(text).await
    }

    /// Copy to clipboard
    pub async fn copy(&self) -> Result<(), AutomationError> {
        debug!("Copying to clipboard");
        self.press_key_combination(&["ctrl".to_string(), "c".to_string()])
            .await
    }

    /// Cut to clipboard
    pub async fn cut(&self) -> Result<(), AutomationError> {
        debug!("Cutting to clipboard");
        self.press_key_combination(&["ctrl".to_string(), "x".to_string()])
            .await
    }

    fn convert_key(&self, key: &str) -> Result<enigo::Key, AutomationError> {
        match key.to_lowercase().as_str() {
            // Special keys
            "enter" | "return" => Ok(enigo::Key::Return),
            "escape" | "esc" => Ok(enigo::Key::Escape),
            "space" => Ok(enigo::Key::Space),
            "tab" => Ok(enigo::Key::Tab),
            "backspace" => Ok(enigo::Key::Backspace),
            "delete" | "del" => Ok(enigo::Key::Delete),
            "home" => Ok(enigo::Key::Home),
            "end" => Ok(enigo::Key::End),
            "pageup" => Ok(enigo::Key::PageUp),
            "pagedown" => Ok(enigo::Key::PageDown),

            // Arrow keys
            "up" | "arrowup" => Ok(enigo::Key::UpArrow),
            "down" | "arrowdown" => Ok(enigo::Key::DownArrow),
            "left" | "arrowleft" => Ok(enigo::Key::LeftArrow),
            "right" | "arrowright" => Ok(enigo::Key::RightArrow),

            // Modifier keys
            "ctrl" | "control" => Ok(enigo::Key::Control),
            "alt" => Ok(enigo::Key::Alt),
            "shift" => Ok(enigo::Key::Shift),
            "meta" | "cmd" | "super" => Ok(enigo::Key::Meta),

            // Function keys
            "f1" => Ok(enigo::Key::F1),
            "f2" => Ok(enigo::Key::F2),
            "f3" => Ok(enigo::Key::F3),
            "f4" => Ok(enigo::Key::F4),
            "f5" => Ok(enigo::Key::F5),
            "f6" => Ok(enigo::Key::F6),
            "f7" => Ok(enigo::Key::F7),
            "f8" => Ok(enigo::Key::F8),
            "f9" => Ok(enigo::Key::F9),
            "f10" => Ok(enigo::Key::F10),
            "f11" => Ok(enigo::Key::F11),
            "f12" => Ok(enigo::Key::F12),

            // Single character keys
            key if key.len() == 1 => {
                let ch = key.chars().next().unwrap();
                Ok(enigo::Key::Unicode(ch))
            }

            _ => Err(AutomationError::InputFailed(format!("Unknown key: {key}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_keyboard_service_creation() {
        let result = KeyboardService::new();
        assert!(
            result.is_ok(),
            "Keyboard service should initialize successfully"
        );
    }

    #[tokio::test]
    async fn test_convert_key() {
        let service = KeyboardService::new().expect("Failed to create keyboard service");

        // Test special keys
        assert!(matches!(
            service.convert_key("enter"),
            Ok(enigo::Key::Return)
        ));
        assert!(matches!(
            service.convert_key("escape"),
            Ok(enigo::Key::Escape)
        ));
        assert!(matches!(
            service.convert_key("ctrl"),
            Ok(enigo::Key::Control)
        ));

        // Test single character
        assert!(matches!(
            service.convert_key("a"),
            Ok(enigo::Key::Unicode('a'))
        ));

        // Test unknown key
        assert!(service.convert_key("unknown_key").is_err());
    }

    #[tokio::test]
    async fn test_empty_text_input() {
        let service = KeyboardService::new().expect("Failed to create keyboard service");
        let result = service.type_text("").await;
        assert!(result.is_err(), "Empty text input should fail with validation error");
    }

    #[tokio::test]
    async fn test_empty_keys_input() {
        let service = KeyboardService::new().expect("Failed to create keyboard service");
        let result = service.press_keys(&[]).await;
        assert!(result.is_err(), "Empty keys input should fail with validation error");
    }

    #[tokio::test]
    async fn test_empty_text_validation() {
        let service = KeyboardService::new().expect("Failed to create keyboard service");
        
        // Test empty text for type_text
        let result = service.type_text("").await;
        assert!(result.is_err(), "Empty text should fail validation");

        // Test empty text for paste_text
        let result = service.paste_text("").await;
        assert!(result.is_err(), "Empty text should fail validation");

        // Test empty text for type_text_with_delay
        let result = service.type_text_with_delay("", 100).await;
        assert!(result.is_err(), "Empty text should fail validation");
    }

    #[tokio::test]
    async fn test_invalid_delay_validation() {
        let service = KeyboardService::new().expect("Failed to create keyboard service");
        
        // Test excessive delay for type_text_with_delay
        let result = service.type_text_with_delay("test", 70000).await;
        assert!(result.is_err(), "Excessive delay should fail validation");

        // Test excessive delay for press_keys_with_delay
        let result = service.press_keys_with_delay(&["a".to_string()], 70000).await;
        assert!(result.is_err(), "Excessive delay should fail validation");
    }

    #[tokio::test]
    async fn test_invalid_keys_validation() {
        let service = KeyboardService::new().expect("Failed to create keyboard service");
        
        // Test empty key in array
        let result = service.press_keys(&["a".to_string(), "".to_string()]).await;
        assert!(result.is_err(), "Empty key should fail validation");

        // Test unknown key
        let result = service.press_keys(&["unknown_key_12345".to_string()]).await;
        assert!(result.is_err(), "Unknown key should fail validation");
    }

    #[tokio::test]
    async fn test_press_keys_with_delay_validation() {
        let service = KeyboardService::new().expect("Failed to create keyboard service");
        
        // Test empty keys array
        let result = service.press_keys_with_delay(&[], 100).await;
        assert!(result.is_err(), "Empty keys array should fail validation");

        // Test valid input (should not fail validation, but might fail execution in headless environment)
        let result = service.press_keys_with_delay(&["a".to_string()], 10).await;
        // We don't assert success here as it might fail in headless CI environment
        // but it should not fail due to validation
        if result.is_err() {
            // If it fails, it should not be due to validation
            let error_msg = result.unwrap_err().to_string();
            assert!(!error_msg.contains("validation"), "Should not fail due to validation: {error_msg}");
        }
    }
}
