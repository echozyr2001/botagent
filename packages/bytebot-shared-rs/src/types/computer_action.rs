use serde::{Deserialize, Serialize};

/// Represents screen coordinates
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Coordinates {
    pub x: i32,
    pub y: i32,
}

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Button {
    Left,
    Right,
    Middle,
}

/// Key press types (up or down)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Press {
    Up,
    Down,
}

/// Scroll direction
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Application types for switching
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Application {
    Firefox,
    #[serde(rename = "1password")]
    OnePassword,
    Thunderbird,
    Vscode,
    Terminal,
    Desktop,
    Directory,
}

/// Move mouse to specific coordinates
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveMouseAction {
    pub action: String, // Always "move_mouse"
    pub coordinates: Coordinates,
}

/// Trace mouse along a path
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceMouseAction {
    pub action: String, // Always "trace_mouse"
    pub path: Vec<Coordinates>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_keys: Option<Vec<String>>,
}

/// Click mouse at coordinates
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClickMouseAction {
    pub action: String, // Always "click_mouse"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<Coordinates>,
    pub button: Button,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_keys: Option<Vec<String>>,
    #[serde(rename = "clickCount")]
    pub click_count: u32,
}

/// Press mouse button (up or down)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PressMouseAction {
    pub action: String, // Always "press_mouse"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<Coordinates>,
    pub button: Button,
    pub press: Press,
}

/// Drag mouse along a path
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DragMouseAction {
    pub action: String, // Always "drag_mouse"
    pub path: Vec<Coordinates>,
    pub button: Button,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_keys: Option<Vec<String>>,
}

/// Scroll at coordinates
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrollAction {
    pub action: String, // Always "scroll"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<Coordinates>,
    pub direction: ScrollDirection,
    #[serde(rename = "scrollCount")]
    pub scroll_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_keys: Option<Vec<String>>,
}

/// Type specific keys
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeKeysAction {
    pub action: String, // Always "type_keys"
    pub keys: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<u64>,
}

/// Paste text from clipboard
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PasteTextAction {
    pub action: String, // Always "paste_text"
    pub text: String,
}

/// Press keys (up or down)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PressKeysAction {
    pub action: String, // Always "press_keys"
    pub keys: Vec<String>,
    pub press: Press,
}

/// Type text with optional delay
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeTextAction {
    pub action: String, // Always "type_text"
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitive: Option<bool>,
}

/// Wait for specified duration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaitAction {
    pub action: String, // Always "wait"
    pub duration: u64,
}

/// Take a screenshot
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotAction {
    pub action: String, // Always "screenshot"
}

/// Get cursor position
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CursorPositionAction {
    pub action: String, // Always "cursor_position"
}

/// Switch to application
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApplicationAction {
    pub action: String, // Always "application"
    pub application: Application,
}

/// Write file with base64 encoded data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WriteFileAction {
    pub action: String, // Always "write_file"
    pub path: String,
    pub data: String, // Base64 encoded data
}

/// Read file
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadFileAction {
    pub action: String, // Always "read_file"
    pub path: String,
}

/// Main computer action enum that encompasses all possible actions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum ComputerAction {
    #[serde(rename = "move_mouse")]
    MoveMouse { coordinates: Coordinates },
    #[serde(rename = "trace_mouse")]
    TraceMouse {
        path: Vec<Coordinates>,
        #[serde(skip_serializing_if = "Option::is_none")]
        hold_keys: Option<Vec<String>>,
    },
    #[serde(rename = "click_mouse")]
    ClickMouse {
        #[serde(skip_serializing_if = "Option::is_none")]
        coordinates: Option<Coordinates>,
        button: Button,
        #[serde(skip_serializing_if = "Option::is_none")]
        hold_keys: Option<Vec<String>>,
        #[serde(rename = "clickCount")]
        click_count: u32,
    },
    #[serde(rename = "press_mouse")]
    PressMouse {
        #[serde(skip_serializing_if = "Option::is_none")]
        coordinates: Option<Coordinates>,
        button: Button,
        press: Press,
    },
    #[serde(rename = "drag_mouse")]
    DragMouse {
        path: Vec<Coordinates>,
        button: Button,
        #[serde(skip_serializing_if = "Option::is_none")]
        hold_keys: Option<Vec<String>>,
    },
    #[serde(rename = "scroll")]
    Scroll {
        #[serde(skip_serializing_if = "Option::is_none")]
        coordinates: Option<Coordinates>,
        direction: ScrollDirection,
        #[serde(rename = "scrollCount")]
        scroll_count: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        hold_keys: Option<Vec<String>>,
    },
    #[serde(rename = "type_keys")]
    TypeKeys {
        keys: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        delay: Option<u64>,
    },
    #[serde(rename = "paste_text")]
    PasteText { text: String },
    #[serde(rename = "press_keys")]
    PressKeys { keys: Vec<String>, press: Press },
    #[serde(rename = "type_text")]
    TypeText {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        delay: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sensitive: Option<bool>,
    },
    #[serde(rename = "wait")]
    Wait { duration: u64 },
    #[serde(rename = "screenshot")]
    Screenshot,
    #[serde(rename = "cursor_position")]
    CursorPosition,
    #[serde(rename = "application")]
    Application { application: Application },
    #[serde(rename = "write_file")]
    WriteFile {
        path: String,
        data: String, // Base64 encoded data
    },
    #[serde(rename = "read_file")]
    ReadFile { path: String },
}

/// Validation error types for computer actions
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ComputerActionValidationError {
    #[error("Invalid coordinates: x={x}, y={y}. Coordinates must be non-negative")]
    InvalidCoordinates { x: i32, y: i32 },

    #[error("Empty path provided for action that requires a path")]
    EmptyPath,

    #[error("Invalid click count: {count}. Must be greater than 0")]
    InvalidClickCount { count: u32 },

    #[error("Invalid scroll count: {count}. Must be greater than 0")]
    InvalidScrollCount { count: u32 },

    #[error("Empty keys array provided")]
    EmptyKeys,

    #[error("Invalid key: '{key}'. Keys cannot be empty")]
    InvalidKey { key: String },

    #[error("Empty text provided")]
    EmptyText,

    #[error("Invalid duration: {duration}. Must be greater than 0")]
    InvalidDuration { duration: u64 },

    #[error("Invalid file path: '{path}'. Path cannot be empty")]
    InvalidFilePath { path: String },

    #[error("Invalid file data: data cannot be empty")]
    InvalidFileData,

    #[error("Invalid delay: {delay}. Delay must be reasonable (< 60000ms)")]
    InvalidDelay { delay: u64 },
}

/// Validation trait for computer actions
pub trait Validate {
    type Error;

    fn validate(&self) -> Result<(), Self::Error>;
}

impl Validate for Coordinates {
    type Error = ComputerActionValidationError;

    fn validate(&self) -> Result<(), Self::Error> {
        if self.x < 0 || self.y < 0 {
            return Err(ComputerActionValidationError::InvalidCoordinates {
                x: self.x,
                y: self.y,
            });
        }
        Ok(())
    }
}

impl Validate for ComputerAction {
    type Error = ComputerActionValidationError;

    fn validate(&self) -> Result<(), Self::Error> {
        match self {
            ComputerAction::MoveMouse { coordinates } => {
                coordinates.validate()?;
            }

            ComputerAction::TraceMouse { path, .. } => {
                if path.is_empty() {
                    return Err(ComputerActionValidationError::EmptyPath);
                }
                for coord in path {
                    coord.validate()?;
                }
            }

            ComputerAction::ClickMouse {
                coordinates,
                click_count,
                ..
            } => {
                if let Some(coords) = coordinates {
                    coords.validate()?;
                }
                if *click_count == 0 {
                    return Err(ComputerActionValidationError::InvalidClickCount {
                        count: *click_count,
                    });
                }
            }

            ComputerAction::PressMouse { coordinates, .. } => {
                if let Some(coords) = coordinates {
                    coords.validate()?;
                }
            }

            ComputerAction::DragMouse { path, .. } => {
                if path.is_empty() {
                    return Err(ComputerActionValidationError::EmptyPath);
                }
                for coord in path {
                    coord.validate()?;
                }
            }

            ComputerAction::Scroll {
                coordinates,
                scroll_count,
                ..
            } => {
                if let Some(coords) = coordinates {
                    coords.validate()?;
                }
                if *scroll_count == 0 {
                    return Err(ComputerActionValidationError::InvalidScrollCount {
                        count: *scroll_count,
                    });
                }
            }

            ComputerAction::TypeKeys { keys, delay } => {
                if keys.is_empty() {
                    return Err(ComputerActionValidationError::EmptyKeys);
                }
                for key in keys {
                    if key.is_empty() {
                        return Err(ComputerActionValidationError::InvalidKey { key: key.clone() });
                    }
                }
                if let Some(d) = delay {
                    if *d > 60000 {
                        return Err(ComputerActionValidationError::InvalidDelay { delay: *d });
                    }
                }
            }

            ComputerAction::PasteText { text } => {
                if text.is_empty() {
                    return Err(ComputerActionValidationError::EmptyText);
                }
            }

            ComputerAction::PressKeys { keys, .. } => {
                if keys.is_empty() {
                    return Err(ComputerActionValidationError::EmptyKeys);
                }
                for key in keys {
                    if key.is_empty() {
                        return Err(ComputerActionValidationError::InvalidKey { key: key.clone() });
                    }
                }
            }

            ComputerAction::TypeText { text, delay, .. } => {
                if text.is_empty() {
                    return Err(ComputerActionValidationError::EmptyText);
                }
                if let Some(d) = delay {
                    if *d > 60000 {
                        return Err(ComputerActionValidationError::InvalidDelay { delay: *d });
                    }
                }
            }

            ComputerAction::Wait { duration } => {
                if *duration == 0 {
                    return Err(ComputerActionValidationError::InvalidDuration {
                        duration: *duration,
                    });
                }
            }

            ComputerAction::WriteFile { path, data } => {
                if path.is_empty() {
                    return Err(ComputerActionValidationError::InvalidFilePath {
                        path: path.clone(),
                    });
                }
                if data.is_empty() {
                    return Err(ComputerActionValidationError::InvalidFileData);
                }
            }

            ComputerAction::ReadFile { path } => {
                if path.is_empty() {
                    return Err(ComputerActionValidationError::InvalidFilePath {
                        path: path.clone(),
                    });
                }
            }

            // These actions don't require validation
            ComputerAction::Screenshot | ComputerAction::CursorPosition => {}

            ComputerAction::Application { .. } => {
                // Application enum variants are already validated by serde
            }
        }

        Ok(())
    }
}

/// Helper functions for creating computer actions with validation
impl ComputerAction {
    /// Create a move mouse action with validation
    pub fn move_mouse(coordinates: Coordinates) -> Result<Self, ComputerActionValidationError> {
        coordinates.validate()?;
        Ok(ComputerAction::MoveMouse { coordinates })
    }

    /// Create a click mouse action with validation
    pub fn click_mouse(
        coordinates: Option<Coordinates>,
        button: Button,
        click_count: u32,
        hold_keys: Option<Vec<String>>,
    ) -> Result<Self, ComputerActionValidationError> {
        if let Some(coords) = &coordinates {
            coords.validate()?;
        }
        if click_count == 0 {
            return Err(ComputerActionValidationError::InvalidClickCount { count: click_count });
        }
        Ok(ComputerAction::ClickMouse {
            coordinates,
            button,
            hold_keys,
            click_count,
        })
    }

    /// Create a type text action with validation
    pub fn type_text(
        text: String,
        delay: Option<u64>,
        sensitive: Option<bool>,
    ) -> Result<Self, ComputerActionValidationError> {
        if text.is_empty() {
            return Err(ComputerActionValidationError::EmptyText);
        }
        if let Some(d) = delay {
            if d > 60000 {
                return Err(ComputerActionValidationError::InvalidDelay { delay: d });
            }
        }
        Ok(ComputerAction::TypeText {
            text,
            delay,
            sensitive,
        })
    }

    /// Create a write file action with validation
    pub fn write_file(path: String, data: String) -> Result<Self, ComputerActionValidationError> {
        if path.is_empty() {
            return Err(ComputerActionValidationError::InvalidFilePath { path });
        }
        if data.is_empty() {
            return Err(ComputerActionValidationError::InvalidFileData);
        }
        Ok(ComputerAction::WriteFile { path, data })
    }

    /// Create a read file action with validation
    pub fn read_file(path: String) -> Result<Self, ComputerActionValidationError> {
        if path.is_empty() {
            return Err(ComputerActionValidationError::InvalidFilePath { path });
        }
        Ok(ComputerAction::ReadFile { path })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinates_validation() {
        let valid_coords = Coordinates { x: 100, y: 200 };
        assert!(valid_coords.validate().is_ok());

        let invalid_coords = Coordinates { x: -1, y: 200 };
        assert!(invalid_coords.validate().is_err());

        let invalid_coords2 = Coordinates { x: 100, y: -1 };
        assert!(invalid_coords2.validate().is_err());
    }

    #[test]
    fn test_move_mouse_validation() {
        let action = ComputerAction::move_mouse(Coordinates { x: 100, y: 200 });
        assert!(action.is_ok());

        let invalid_action = ComputerAction::move_mouse(Coordinates { x: -1, y: 200 });
        assert!(invalid_action.is_err());
    }

    #[test]
    fn test_click_mouse_validation() {
        let action = ComputerAction::click_mouse(
            Some(Coordinates { x: 100, y: 200 }),
            Button::Left,
            1,
            None,
        );
        assert!(action.is_ok());

        let invalid_action = ComputerAction::click_mouse(
            Some(Coordinates { x: 100, y: 200 }),
            Button::Left,
            0,
            None,
        );
        assert!(invalid_action.is_err());
    }

    #[test]
    fn test_type_text_validation() {
        let action = ComputerAction::type_text("Hello".to_string(), Some(100), None);
        assert!(action.is_ok());

        let invalid_action = ComputerAction::type_text("".to_string(), None, None);
        assert!(invalid_action.is_err());

        let invalid_delay = ComputerAction::type_text("Hello".to_string(), Some(70000), None);
        assert!(invalid_delay.is_err());
    }

    #[test]
    fn test_file_operations_validation() {
        let write_action =
            ComputerAction::write_file("/path/to/file".to_string(), "base64data".to_string());
        assert!(write_action.is_ok());

        let invalid_write = ComputerAction::write_file("".to_string(), "data".to_string());
        assert!(invalid_write.is_err());

        let read_action = ComputerAction::read_file("/path/to/file".to_string());
        assert!(read_action.is_ok());

        let invalid_read = ComputerAction::read_file("".to_string());
        assert!(invalid_read.is_err());
    }

    #[test]
    fn test_serialization() {
        let action = ComputerAction::MoveMouse {
            coordinates: Coordinates { x: 100, y: 200 },
        };

        let json = serde_json::to_string(&action).unwrap();
        let deserialized: ComputerAction = serde_json::from_str(&json).unwrap();

        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_complex_action_validation() {
        let trace_action = ComputerAction::TraceMouse {
            path: vec![Coordinates { x: 0, y: 0 }, Coordinates { x: 100, y: 100 }],
            hold_keys: Some(vec!["ctrl".to_string()]),
        };
        assert!(trace_action.validate().is_ok());

        let invalid_trace = ComputerAction::TraceMouse {
            path: vec![],
            hold_keys: None,
        };
        assert!(invalid_trace.validate().is_err());
    }

    #[test]
    fn test_json_format_compatibility() {
        // Test move_mouse action
        let move_action = ComputerAction::MoveMouse {
            coordinates: Coordinates { x: 100, y: 200 },
        };
        let json = serde_json::to_string(&move_action).unwrap();
        println!("MoveMouse JSON: {json}");
        assert!(json.contains(r#""action":"move_mouse""#));
        assert!(json.contains(r#""coordinates":{"x":100,"y":200}"#));

        // Test click_mouse action
        let click_action = ComputerAction::ClickMouse {
            coordinates: Some(Coordinates { x: 50, y: 75 }),
            button: Button::Left,
            hold_keys: Some(vec!["ctrl".to_string()]),
            click_count: 2,
        };
        let json = serde_json::to_string(&click_action).unwrap();
        println!("ClickMouse JSON: {json}");
        assert!(json.contains(r#""action":"click_mouse""#));
        assert!(json.contains(r#""button":"left""#));
        assert!(json.contains(r#""clickCount":2"#));

        // Test type_text action
        let type_action = ComputerAction::TypeText {
            text: "Hello World".to_string(),
            delay: Some(100),
            sensitive: Some(true),
        };
        let json = serde_json::to_string(&type_action).unwrap();
        println!("TypeText JSON: {json}");
        assert!(json.contains(r#""action":"type_text""#));
        assert!(json.contains(r#""text":"Hello World""#));
        assert!(json.contains(r#""delay":100"#));
        assert!(json.contains(r#""sensitive":true"#));

        // Test application action
        let app_action = ComputerAction::Application {
            application: Application::Firefox,
        };
        let json = serde_json::to_string(&app_action).unwrap();
        println!("Application JSON: {json}");
        assert!(json.contains(r#""action":"application""#));
        assert!(json.contains(r#""application":"firefox""#));

        // Test screenshot action (no additional fields)
        let screenshot_action = ComputerAction::Screenshot;
        let json = serde_json::to_string(&screenshot_action).unwrap();
        println!("Screenshot JSON: {json}");
        assert!(json.contains(r#""action":"screenshot""#));
    }

    #[test]
    fn test_deserialization_from_typescript_format() {
        // Test deserializing from TypeScript-style JSON
        let move_json = r#"{"action":"move_mouse","coordinates":{"x":100,"y":200}}"#;
        let action: ComputerAction = serde_json::from_str(move_json).unwrap();
        match action {
            ComputerAction::MoveMouse { coordinates } => {
                assert_eq!(coordinates.x, 100);
                assert_eq!(coordinates.y, 200);
            }
            _ => panic!("Expected MoveMouse action"),
        }

        let click_json = r#"{"action":"click_mouse","coordinates":{"x":50,"y":75},"button":"left","clickCount":1}"#;
        let action: ComputerAction = serde_json::from_str(click_json).unwrap();
        match action {
            ComputerAction::ClickMouse {
                coordinates,
                button,
                click_count,
                ..
            } => {
                assert_eq!(coordinates.unwrap(), Coordinates { x: 50, y: 75 });
                assert_eq!(button, Button::Left);
                assert_eq!(click_count, 1);
            }
            _ => panic!("Expected ClickMouse action"),
        }

        let type_json = r#"{"action":"type_text","text":"Hello","delay":50}"#;
        let action: ComputerAction = serde_json::from_str(type_json).unwrap();
        match action {
            ComputerAction::TypeText { text, delay, .. } => {
                assert_eq!(text, "Hello");
                assert_eq!(delay, Some(50));
            }
            _ => panic!("Expected TypeText action"),
        }
    }
}
