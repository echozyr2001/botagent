use std::sync::Arc;

use axum::{extract::State, response::Json};
use base64::Engine;
use bytebot_shared_rs::types::computer_action::{Application, ComputerAction, Validate};
use serde_json::{json, Value};
use tracing::{debug, error, info, warn};

use crate::{
    automation::{AutomationService, ComputerAutomation},
    error::ServiceError,
};

/// Handle computer automation actions
pub async fn handle_computer_action(
    State(automation_service): State<Arc<AutomationService>>,
    Json(action): Json<ComputerAction>,
) -> Result<Json<Value>, ServiceError> {
    info!("Received computer action: {:?}", action);

    // Validate the action before processing
    if let Err(validation_error) = action.validate() {
        warn!("Invalid computer action received: {}", validation_error);
        return Err(ServiceError::Automation(crate::error::AutomationError::Validation(
            validation_error.to_string(),
        )));
    }

    let result = match action {
        ComputerAction::Screenshot => {
            debug!("Taking screenshot");
            match automation_service.take_screenshot().await {
                Ok(screenshot) => {
                    info!("Successfully captured screenshot ({} bytes)", screenshot.len());
                    json!({
                        "success": true,
                        "action": "screenshot",
                        "result": {
                            "screenshot": screenshot,
                            "size": screenshot.len()
                        }
                    })
                }
                Err(e) => {
                    error!("Failed to take screenshot: {}", e);
                    return Err(ServiceError::Automation(e));
                }
            }
        }

        ComputerAction::MoveMouse { coordinates } => {
            debug!("Moving mouse to ({}, {})", coordinates.x, coordinates.y);
            match automation_service.move_mouse(coordinates).await {
                Ok(()) => {
                    debug!("Successfully moved mouse to ({}, {})", coordinates.x, coordinates.y);
                    json!({
                        "success": true,
                        "action": "move_mouse",
                        "result": {
                            "coordinates": coordinates
                        }
                    })
                }
                Err(e) => {
                    error!("Failed to move mouse to ({}, {}): {}", coordinates.x, coordinates.y, e);
                    return Err(ServiceError::Automation(e));
                }
            }
        }

        ComputerAction::ClickMouse {
            coordinates,
            button,
            click_count,
            hold_keys,
        } => {
            debug!(
                "Clicking mouse at {:?} with {:?} button, {} times",
                coordinates, button, click_count
            );

            automation_service
                .click_mouse_with_options(coordinates, button, click_count, hold_keys.as_deref())
                .await?;

            json!({
                "success": true,
                "action": "click_mouse",
                "result": {
                    "coordinates": coordinates,
                    "button": button,
                    "click_count": click_count
                }
            })
        }

        ComputerAction::TypeText { text, delay, .. } => {
            debug!("Typing text: {} (delay: {:?}ms)", text, delay);

            if let Some(delay_ms) = delay {
                automation_service
                    .type_text_with_delay(&text, delay_ms)
                    .await?;
            } else {
                automation_service.type_text(&text).await?;
            }

            json!({
                "success": true,
                "action": "type_text",
                "result": {
                    "text": text,
                    "delay": delay
                }
            })
        }

        ComputerAction::PressKeys { keys, press } => {
            debug!("Pressing keys: {:?} with press type: {:?}", keys, press);

            automation_service
                .press_keys_with_type(&keys, press)
                .await?;

            json!({
                "success": true,
                "action": "press_keys",
                "result": {
                    "keys": keys,
                    "press": press
                }
            })
        }

        ComputerAction::TypeKeys { keys, delay } => {
            debug!("Typing keys: {:?} (delay: {:?}ms)", keys, delay);

            if let Some(delay_ms) = delay {
                automation_service
                    .press_keys_with_delay(&keys, delay_ms)
                    .await?;
            } else {
                automation_service.press_keys(&keys).await?;
            }

            json!({
                "success": true,
                "action": "type_keys",
                "result": {
                    "keys": keys,
                    "delay": delay
                }
            })
        }

        ComputerAction::PasteText { text } => {
            debug!("Pasting text: {}", text);
            automation_service.paste_text(&text).await?;
            json!({
                "success": true,
                "action": "paste_text",
                "result": {
                    "text": text
                }
            })
        }

        ComputerAction::ReadFile { path } => {
            debug!("Reading file: {}", path);
            
            // Enhanced validation for file operations
            if path.is_empty() {
                error!("Empty file path provided");
                return Err(ServiceError::Automation(crate::error::AutomationError::InvalidPath(
                    "File path cannot be empty".to_string(),
                )));
            }

            if path.len() > 4096 {
                error!("File path too long: {} characters", path.len());
                return Err(ServiceError::Automation(crate::error::AutomationError::InvalidPath(
                    "File path too long (maximum 4096 characters)".to_string(),
                )));
            }

            // Check for suspicious patterns in path
            let suspicious_patterns = ["../", "..\\", "~", "$", "`", ";", "|", "&"];
            for pattern in &suspicious_patterns {
                if path.contains(pattern) {
                    warn!("Suspicious pattern '{}' detected in file path: {}", pattern, path);
                    return Err(ServiceError::Automation(crate::error::AutomationError::InvalidPath(
                        format!("Suspicious pattern '{pattern}' not allowed in file path"),
                    )));
                }
            }

            match automation_service.read_file(&path).await {
                Ok(content) => {
                    info!("Successfully read file: {} ({} bytes base64)", path, content.len());
                    
                    // Log file type information for monitoring
                    let file_extension = std::path::Path::new(&path)
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or("unknown");
                    
                    debug!("File type: .{}", file_extension);
                    
                    json!({
                        "success": true,
                        "action": "read_file",
                        "result": {
                            "path": path,
                            "content": content,
                            "size": content.len(),
                            "file_type": file_extension,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }
                    })
                }
                Err(e) => {
                    error!("Failed to read file {}: {}", path, e);
                    return Err(ServiceError::Automation(e));
                }
            }
        }

        ComputerAction::WriteFile { path, data } => {
            debug!("Writing file: {} ({} bytes base64)", path, data.len());
            
            // Enhanced validation for file operations
            if path.is_empty() {
                error!("Empty file path provided");
                return Err(ServiceError::Automation(crate::error::AutomationError::InvalidPath(
                    "File path cannot be empty".to_string(),
                )));
            }

            if path.len() > 4096 {
                error!("File path too long: {} characters", path.len());
                return Err(ServiceError::Automation(crate::error::AutomationError::InvalidPath(
                    "File path too long (maximum 4096 characters)".to_string(),
                )));
            }

            if data.is_empty() {
                error!("Empty data provided for file write");
                return Err(ServiceError::Automation(crate::error::AutomationError::Validation(
                    "Cannot write empty data to file".to_string(),
                )));
            }

            // Check for suspicious patterns in path
            let suspicious_patterns = ["../", "..\\", "~", "$", "`", ";", "|", "&"];
            for pattern in &suspicious_patterns {
                if path.contains(pattern) {
                    warn!("Suspicious pattern '{}' detected in file path: {}", pattern, path);
                    return Err(ServiceError::Automation(crate::error::AutomationError::InvalidPath(
                        format!("Suspicious pattern '{pattern}' not allowed in file path"),
                    )));
                }
            }

            // Validate base64 data before processing
            let decoded_size = match base64::engine::general_purpose::STANDARD.decode(&data) {
                Ok(decoded) => {
                    debug!("Base64 data decoded successfully ({} bytes)", decoded.len());
                    decoded.len()
                }
                Err(decode_err) => {
                    error!("Invalid base64 data for file {}: {}", path, decode_err);
                    return Err(ServiceError::Automation(crate::error::AutomationError::Validation(
                        format!("Invalid base64 data: {decode_err}"),
                    )));
                }
            };

            // Additional size validation
            if decoded_size == 0 {
                return Err(ServiceError::Automation(crate::error::AutomationError::Validation(
                    "Decoded file content is empty".to_string(),
                )));
            }

            // Log file type information for monitoring
            let file_extension = std::path::Path::new(&path)
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("unknown");
            
            debug!("Writing file type: .{}", file_extension);

            match automation_service.write_file(&path, &data).await {
                Ok(()) => {
                    info!("Successfully wrote file: {} ({} bytes base64, {} bytes decoded)", 
                          path, data.len(), decoded_size);
                    json!({
                        "success": true,
                        "action": "write_file",
                        "result": {
                            "path": path,
                            "bytes_written_base64": data.len(),
                            "bytes_written_decoded": decoded_size,
                            "file_type": file_extension,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }
                    })
                }
                Err(e) => {
                    error!("Failed to write file {}: {}", path, e);
                    return Err(ServiceError::Automation(e));
                }
            }
        }

        ComputerAction::Wait { duration } => {
            debug!("Waiting for {} milliseconds", duration);
            tokio::time::sleep(tokio::time::Duration::from_millis(duration)).await;
            json!({
                "success": true,
                "action": "wait",
                "result": {
                    "duration": duration
                }
            })
        }

        ComputerAction::CursorPosition => {
            debug!("Getting cursor position");
            let position = automation_service.get_cursor_position().await?;
            json!({
                "success": true,
                "action": "cursor_position",
                "result": {
                    "coordinates": position
                }
            })
        }

        ComputerAction::Scroll {
            coordinates,
            direction,
            scroll_count,
            hold_keys,
        } => {
            debug!(
                "Scrolling {:?} {} times at coordinates: {:?}",
                direction, scroll_count, coordinates
            );

            automation_service
                .scroll(coordinates, direction, scroll_count, hold_keys.as_deref())
                .await?;

            json!({
                "success": true,
                "action": "scroll",
                "result": {
                    "coordinates": coordinates,
                    "direction": direction,
                    "scroll_count": scroll_count
                }
            })
        }

        ComputerAction::TraceMouse { path, hold_keys } => {
            debug!("Tracing mouse along path with {} points", path.len());

            automation_service
                .trace_mouse_path(&path, hold_keys.as_deref())
                .await?;

            json!({
                "success": true,
                "action": "trace_mouse",
                "result": {
                    "path_length": path.len()
                }
            })
        }

        ComputerAction::PressMouse {
            coordinates,
            button,
            press,
        } => {
            debug!(
                "Pressing mouse button {:?} with press type {:?} at coordinates: {:?}",
                button, press, coordinates
            );

            automation_service
                .press_mouse(coordinates, button, press)
                .await?;

            json!({
                "success": true,
                "action": "press_mouse",
                "result": {
                    "coordinates": coordinates,
                    "button": button,
                    "press": press
                }
            })
        }

        ComputerAction::DragMouse {
            path,
            button,
            hold_keys,
        } => {
            debug!(
                "Dragging mouse along path with {} points using {:?} button",
                path.len(),
                button
            );

            automation_service
                .drag_mouse_path(&path, button, hold_keys.as_deref())
                .await?;

            json!({
                "success": true,
                "action": "drag_mouse",
                "result": {
                    "path_length": path.len(),
                    "button": button
                }
            })
        }

        ComputerAction::Application { application } => {
            debug!("Switching to application: {:?}", application);

            let app_name = match application {
                Application::Firefox => "firefox",
                Application::OnePassword => "1password",
                Application::Thunderbird => "thunderbird",
                Application::Vscode => "vscode",
                Application::Terminal => "terminal",
                Application::Desktop => "desktop",
                Application::Directory => "directory",
            };

            match automation_service.applications.switch_to(app_name).await {
                Ok(()) => {
                    info!("Successfully switched to application: {:?}", application);
                    json!({
                        "success": true,
                        "action": "application",
                        "result": {
                            "application": application
                        }
                    })
                }
                Err(e) => {
                    error!("Failed to switch to application {:?}: {}", application, e);
                    return Err(ServiceError::Automation(e));
                }
            }
        }
    };

    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::AutomationService;
    use bytebot_shared_rs::types::computer_action::Coordinates;

    #[tokio::test]
    async fn test_screenshot_action() {
        let automation_service =
            Arc::new(AutomationService::new().expect("Failed to create automation service"));

        let action = ComputerAction::Screenshot;
        let result = handle_computer_action(State(automation_service), Json(action)).await;

        // In headless environments or CI, screenshot might fail
        // This is expected behavior, so we handle both cases
        match result {
            Ok(response) => {
                let response = response.0;
                assert_eq!(response["success"], true);
                assert_eq!(response["action"], "screenshot");
                assert!(response["result"]["screenshot"].is_string());
            }
            Err(e) => {
                // In headless/CI environments, this is expected
                println!("Screenshot action failed (expected in headless environment): {e}");
                // We don't fail the test, just log the expected failure
            }
        }
    }

    #[tokio::test]
    async fn test_move_mouse_action() {
        let automation_service =
            Arc::new(AutomationService::new().expect("Failed to create automation service"));

        let coordinates = Coordinates { x: 100, y: 200 };
        let action = ComputerAction::MoveMouse { coordinates };

        let result = handle_computer_action(State(automation_service), Json(action)).await;

        assert!(result.is_ok(), "Move mouse action should succeed");

        let response = result.unwrap().0;
        assert_eq!(response["success"], true);
        assert_eq!(response["action"], "move_mouse");
        assert_eq!(response["result"]["coordinates"]["x"], 100);
        assert_eq!(response["result"]["coordinates"]["y"], 200);
    }

    #[tokio::test]
    async fn test_click_mouse_action_with_options() {
        let automation_service =
            Arc::new(AutomationService::new().expect("Failed to create automation service"));

        let coordinates = Some(Coordinates { x: 150, y: 250 });
        let action = ComputerAction::ClickMouse {
            coordinates,
            button: bytebot_shared_rs::types::computer_action::Button::Left,
            click_count: 2,
            hold_keys: Some(vec!["ctrl".to_string()]),
        };

        let result = handle_computer_action(State(automation_service), Json(action)).await;

        assert!(result.is_ok(), "Click mouse action should succeed");

        let response = result.unwrap().0;
        assert_eq!(response["success"], true);
        assert_eq!(response["action"], "click_mouse");
        assert_eq!(response["result"]["click_count"], 2);
    }

    #[tokio::test]
    async fn test_type_text_with_delay_action() {
        let automation_service =
            Arc::new(AutomationService::new().expect("Failed to create automation service"));

        let action = ComputerAction::TypeText {
            text: "Hello World".to_string(),
            delay: Some(50),
            sensitive: Some(false),
        };

        let result = handle_computer_action(State(automation_service), Json(action)).await;

        assert!(result.is_ok(), "Type text action should succeed");

        let response = result.unwrap().0;
        assert_eq!(response["success"], true);
        assert_eq!(response["action"], "type_text");
        assert_eq!(response["result"]["text"], "Hello World");
        assert_eq!(response["result"]["delay"], 50);
    }

    #[tokio::test]
    async fn test_paste_text_action() {
        let automation_service =
            Arc::new(AutomationService::new().expect("Failed to create automation service"));

        let action = ComputerAction::PasteText {
            text: "Pasted content".to_string(),
        };

        let result = handle_computer_action(State(automation_service), Json(action)).await;

        assert!(result.is_ok(), "Paste text action should succeed");

        let response = result.unwrap().0;
        assert_eq!(response["success"], true);
        assert_eq!(response["action"], "paste_text");
        assert_eq!(response["result"]["text"], "Pasted content");
    }

    #[tokio::test]
    async fn test_scroll_action() {
        let automation_service =
            Arc::new(AutomationService::new().expect("Failed to create automation service"));

        let action = ComputerAction::Scroll {
            coordinates: Some(Coordinates { x: 300, y: 400 }),
            direction: bytebot_shared_rs::types::computer_action::ScrollDirection::Up,
            scroll_count: 3,
            hold_keys: None,
        };

        let result = handle_computer_action(State(automation_service), Json(action)).await;

        assert!(result.is_ok(), "Scroll action should succeed");

        let response = result.unwrap().0;
        assert_eq!(response["success"], true);
        assert_eq!(response["action"], "scroll");
        assert_eq!(response["result"]["scroll_count"], 3);
    }

    #[tokio::test]
    async fn test_press_keys_action() {
        let automation_service =
            Arc::new(AutomationService::new().expect("Failed to create automation service"));

        let action = ComputerAction::PressKeys {
            keys: vec!["ctrl".to_string(), "c".to_string()],
            press: bytebot_shared_rs::types::computer_action::Press::Down,
        };

        let result = handle_computer_action(State(automation_service), Json(action)).await;

        assert!(result.is_ok(), "Press keys action should succeed");

        let response = result.unwrap().0;
        assert_eq!(response["success"], true);
        assert_eq!(response["action"], "press_keys");
        assert_eq!(response["result"]["keys"][0], "ctrl");
        assert_eq!(response["result"]["keys"][1], "c");
    }

    #[tokio::test]
    async fn test_drag_mouse_action() {
        let automation_service =
            Arc::new(AutomationService::new().expect("Failed to create automation service"));

        let path = vec![
            Coordinates { x: 100, y: 100 },
            Coordinates { x: 200, y: 200 },
            Coordinates { x: 300, y: 300 },
        ];

        let action = ComputerAction::DragMouse {
            path,
            button: bytebot_shared_rs::types::computer_action::Button::Left,
            hold_keys: None,
        };

        let result = handle_computer_action(State(automation_service), Json(action)).await;

        assert!(result.is_ok(), "Drag mouse action should succeed");

        let response = result.unwrap().0;
        assert_eq!(response["success"], true);
        assert_eq!(response["action"], "drag_mouse");
        assert_eq!(response["result"]["path_length"], 3);
    }

    #[tokio::test]
    async fn test_trace_mouse_action() {
        let automation_service =
            Arc::new(AutomationService::new().expect("Failed to create automation service"));

        let path = vec![Coordinates { x: 50, y: 50 }, Coordinates { x: 100, y: 100 }];

        let action = ComputerAction::TraceMouse {
            path,
            hold_keys: None,
        };

        let result = handle_computer_action(State(automation_service), Json(action)).await;

        assert!(result.is_ok(), "Trace mouse action should succeed");

        let response = result.unwrap().0;
        assert_eq!(response["success"], true);
        assert_eq!(response["action"], "trace_mouse");
        assert_eq!(response["result"]["path_length"], 2);
    }

    #[tokio::test]
    async fn test_application_switching_action() {
        let automation_service =
            Arc::new(AutomationService::new().expect("Failed to create automation service"));

        let action = ComputerAction::Application {
            application: Application::Firefox,
        };

        let result = handle_computer_action(State(automation_service), Json(action)).await;

        // Application switching may fail in test environment, but should not panic
        match result {
            Ok(response) => {
                let response = response.0;
                assert_eq!(response["success"], true);
                assert_eq!(response["action"], "application");
                assert_eq!(response["result"]["application"], "firefox");
            }
            Err(e) => {
                // In test environment, this is expected to fail
                println!("Application switching failed (expected in test env): {e}");
                // Verify it's the correct error type
                match e {
                    ServiceError::Automation(crate::error::AutomationError::ApplicationFailed(_)) => {
                        // This is the expected error type
                    }
                    _ => panic!("Expected ApplicationFailed error, got: {e:?}"),
                }
            }
        }
    }

    #[tokio::test]
    async fn test_file_operations_through_api() {
        let automation_service =
            Arc::new(AutomationService::new().expect("Failed to create automation service"));

        // Test writing a file
        let test_content = "Hello, World!";
        let base64_content = base64::engine::general_purpose::STANDARD.encode(test_content.as_bytes());
        let test_file_path = "./test_api_file.txt";

        let write_action = ComputerAction::WriteFile {
            path: test_file_path.to_string(),
            data: base64_content.clone(),
        };

        let write_result = handle_computer_action(State(automation_service.clone()), Json(write_action)).await;
        assert!(write_result.is_ok(), "Write file action should succeed");

        let write_response = write_result.unwrap().0;
        assert_eq!(write_response["success"], true);
        assert_eq!(write_response["action"], "write_file");
        assert_eq!(write_response["result"]["path"], test_file_path);
        assert_eq!(write_response["result"]["bytes_written_base64"], base64_content.len());

        // Test reading the file back
        let read_action = ComputerAction::ReadFile {
            path: test_file_path.to_string(),
        };

        let read_result = handle_computer_action(State(automation_service), Json(read_action)).await;
        assert!(read_result.is_ok(), "Read file action should succeed");

        let read_response = read_result.unwrap().0;
        assert_eq!(read_response["success"], true);
        assert_eq!(read_response["action"], "read_file");
        assert_eq!(read_response["result"]["path"], test_file_path);
        
        // Verify the content matches
        let returned_content = read_response["result"]["content"].as_str().unwrap();
        assert_eq!(returned_content, base64_content);

        // Clean up
        let _ = tokio::fs::remove_file(test_file_path).await;
    }

    #[tokio::test]
    async fn test_file_validation_errors() {
        let automation_service =
            Arc::new(AutomationService::new().expect("Failed to create automation service"));

        // Test empty path
        let empty_path_action = ComputerAction::ReadFile {
            path: "".to_string(),
        };

        let result = handle_computer_action(State(automation_service.clone()), Json(empty_path_action)).await;
        assert!(result.is_err(), "Empty path should fail");

        // Test suspicious path
        let suspicious_path_action = ComputerAction::ReadFile {
            path: "../../../etc/passwd".to_string(),
        };

        let result = handle_computer_action(State(automation_service.clone()), Json(suspicious_path_action)).await;
        assert!(result.is_err(), "Suspicious path should fail");

        // Test invalid base64 data
        let invalid_base64_action = ComputerAction::WriteFile {
            path: "./test.txt".to_string(),
            data: "invalid-base64-data!@#$%".to_string(),
        };

        let result = handle_computer_action(State(automation_service), Json(invalid_base64_action)).await;
        assert!(result.is_err(), "Invalid base64 data should fail");
    }

    #[tokio::test]
    async fn test_cursor_position_action() {
        let automation_service =
            Arc::new(AutomationService::new().expect("Failed to create automation service"));

        let action = ComputerAction::CursorPosition;

        let result = handle_computer_action(State(automation_service), Json(action)).await;

        // Cursor position may fail in headless environment, but should not panic
        match result {
            Ok(response) => {
                let response = response.0;
                assert_eq!(response["success"], true);
                assert_eq!(response["action"], "cursor_position");
                assert!(response["result"]["coordinates"].is_object());
            }
            Err(e) => {
                // In headless/test environment, this is expected
                println!("Cursor position failed (expected in headless environment): {e}");
            }
        }
    }
}
