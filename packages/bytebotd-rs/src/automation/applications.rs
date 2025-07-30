use std::process::Command;

use bytebot_shared_rs::types::computer_action::Application;
use tracing::{debug, info, warn};

use crate::error::AutomationError;

#[derive(Debug, Clone)]
pub struct ApplicationService;

impl ApplicationService {
    pub fn new() -> Result<Self, AutomationError> {
        Ok(Self)
    }

    /// Switch to a specific application using the Application enum
    pub async fn switch_to_application(&self, application: Application) -> Result<(), AutomationError> {
        let app_name = match application {
            Application::Firefox => "firefox",
            Application::OnePassword => "1password",
            Application::Thunderbird => "thunderbird",
            Application::Vscode => "vscode",
            Application::Terminal => "terminal",
            Application::Desktop => "desktop",
            Application::Directory => "directory",
        };
        
        self.switch_to(app_name).await
    }

    /// Switch to a specific application
    pub async fn switch_to(&self, application: &str) -> Result<(), AutomationError> {
        debug!("Switching to application: {}", application);

        match application.to_lowercase().as_str() {
            "firefox" => self.switch_to_firefox().await,
            "vscode" | "code" => self.switch_to_vscode().await,
            "terminal" => self.switch_to_terminal().await,
            "desktop" => self.switch_to_desktop().await,
            "directory" | "files" => self.switch_to_directory().await,
            "1password" => self.switch_to_1password().await,
            "thunderbird" => self.switch_to_thunderbird().await,
            _ => {
                warn!("Unknown application: {}", application);
                Err(AutomationError::ApplicationFailed(format!(
                    "Application '{application}' is not supported"
                )))
            }
        }
    }

    async fn switch_to_firefox(&self) -> Result<(), AutomationError> {
        debug!("Switching to Firefox");
        
        // Try to find and focus existing Firefox windows first
        if self.focus_existing_window("firefox").await.is_ok() {
            info!("Successfully focused existing Firefox window");
            return Ok(());
        }

        // If no existing window found, try to launch Firefox
        info!("No existing Firefox window found, attempting to launch");
        self.launch_application("firefox", &["firefox"]).await
    }

    async fn switch_to_vscode(&self) -> Result<(), AutomationError> {
        debug!("Switching to VS Code");
        
        // Try to find and focus existing VS Code windows first
        if self.focus_existing_window("code").await.is_ok() {
            info!("Successfully focused existing VS Code window");
            return Ok(());
        }

        // If no existing window found, try to launch VS Code
        info!("No existing VS Code window found, attempting to launch");
        self.launch_application("code", &["code", "code-oss", "/usr/bin/code"]).await
    }

    async fn switch_to_terminal(&self) -> Result<(), AutomationError> {
        debug!("Switching to terminal");
        
        // Try to find and focus existing terminal windows first
        if self.focus_existing_window("terminal").await.is_ok() {
            info!("Successfully focused existing terminal window");
            return Ok(());
        }

        // If no existing window found, try to launch terminal
        info!("No existing terminal window found, attempting to launch");
        self.launch_application("terminal", &["xfce4-terminal", "gnome-terminal", "konsole", "xterm"]).await
    }

    async fn switch_to_desktop(&self) -> Result<(), AutomationError> {
        debug!("Switching to desktop");
        
        // For desktop switching, we minimize all windows or use desktop shortcut
        match self.minimize_all_windows().await {
            Ok(()) => {
                info!("Successfully minimized all windows to show desktop");
                Ok(())
            }
            Err(_) => {
                // Fallback: try using desktop keyboard shortcut
                warn!("Failed to minimize windows, trying desktop shortcut");
                self.send_desktop_shortcut().await
            }
        }
    }

    async fn switch_to_directory(&self) -> Result<(), AutomationError> {
        debug!("Switching to file manager");
        
        // Try to find and focus existing file manager windows first
        if self.focus_existing_window("thunar").await.is_ok() 
            || self.focus_existing_window("nautilus").await.is_ok()
            || self.focus_existing_window("dolphin").await.is_ok() {
            info!("Successfully focused existing file manager window");
            return Ok(());
        }

        // If no existing window found, try to launch file manager
        info!("No existing file manager window found, attempting to launch");
        self.launch_application("file manager", &["thunar", "nautilus", "dolphin", "pcmanfm"]).await
    }

    async fn switch_to_1password(&self) -> Result<(), AutomationError> {
        debug!("Switching to 1Password");
        
        // Try to find and focus existing 1Password windows first
        if self.focus_existing_window("1password").await.is_ok() {
            info!("Successfully focused existing 1Password window");
            return Ok(());
        }

        // If no existing window found, try to launch 1Password
        info!("No existing 1Password window found, attempting to launch");
        self.launch_application("1password", &["1password", "/opt/1Password/1password"]).await
    }

    async fn switch_to_thunderbird(&self) -> Result<(), AutomationError> {
        debug!("Switching to Thunderbird");
        
        // Try to find and focus existing Thunderbird windows first
        if self.focus_existing_window("thunderbird").await.is_ok() {
            info!("Successfully focused existing Thunderbird window");
            return Ok(());
        }

        // If no existing window found, try to launch Thunderbird
        info!("No existing Thunderbird window found, attempting to launch");
        self.launch_application("thunderbird", &["thunderbird"]).await
    }

    /// Try to focus an existing window by application name
    async fn focus_existing_window(&self, app_name: &str) -> Result<(), AutomationError> {
        debug!("Attempting to focus existing window for: {}", app_name);

        // Use wmctrl to find and focus windows if available
        let output = tokio::task::spawn_blocking(move || {
            Command::new("wmctrl")
                .args(["-l"])
                .output()
        }).await.map_err(|e| {
            AutomationError::ApplicationFailed(format!("Failed to execute wmctrl command: {e}"))
        })?;

        match output {
            Ok(output) if output.status.success() => {
                let window_list = String::from_utf8_lossy(&output.stdout);
                debug!("Window list: {}", window_list);

                // Look for windows containing the application name
                for line in window_list.lines() {
                    if line.to_lowercase().contains(app_name) {
                        // Extract window ID (first column)
                        if let Some(window_id) = line.split_whitespace().next() {
                            debug!("Found window ID: {} for app: {}", window_id, app_name);
                            return self.focus_window_by_id(window_id).await;
                        }
                    }
                }
                
                Err(AutomationError::ApplicationFailed(format!(
                    "No existing window found for application: {app_name}"
                )))
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(AutomationError::ApplicationFailed(format!(
                    "wmctrl command failed: {stderr}"
                )))
            }
            Err(e) => {
                warn!("wmctrl not available or failed: {}", e);
                // Try alternative method using xdotool
                self.focus_window_with_xdotool(app_name).await
            }
        }
    }

    /// Focus a window by its ID using wmctrl
    async fn focus_window_by_id(&self, window_id: &str) -> Result<(), AutomationError> {
        debug!("Focusing window with ID: {}", window_id);

        let window_id_clone = window_id.to_string();
        let output = tokio::task::spawn_blocking(move || {
            Command::new("wmctrl")
                .args(["-i", "-a", &window_id_clone])
                .output()
        }).await.map_err(|e| {
            AutomationError::ApplicationFailed(format!("Failed to execute wmctrl focus command: {e}"))
        })?;

        match output {
            Ok(output) if output.status.success() => {
                info!("Successfully focused window: {}", window_id);
                Ok(())
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(AutomationError::ApplicationFailed(format!(
                    "Failed to focus window {window_id}: {stderr}"
                )))
            }
            Err(e) => Err(AutomationError::ApplicationFailed(format!(
                "Failed to focus window {window_id}: {e}"
            )))
        }
    }

    /// Try to focus window using xdotool as fallback
    async fn focus_window_with_xdotool(&self, app_name: &str) -> Result<(), AutomationError> {
        debug!("Attempting to focus window using xdotool for: {}", app_name);

        let app_name_clone = app_name.to_string();
        let output = tokio::task::spawn_blocking(move || {
            Command::new("xdotool")
                .args(["search", "--name", &app_name_clone, "windowactivate"])
                .output()
        }).await.map_err(|e| {
            AutomationError::ApplicationFailed(format!("Failed to execute xdotool command: {e}"))
        })?;

        match output {
            Ok(output) if output.status.success() => {
                info!("Successfully focused window using xdotool for: {}", app_name);
                Ok(())
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(AutomationError::ApplicationFailed(format!(
                    "xdotool failed to focus window for {app_name}: {stderr}"
                )))
            }
            Err(e) => Err(AutomationError::ApplicationFailed(format!(
                "Failed to use xdotool for {app_name}: {e}"
            )))
        }
    }

    /// Launch an application using one of the provided command variants
    async fn launch_application(&self, app_name: &str, commands: &[&str]) -> Result<(), AutomationError> {
        debug!("Attempting to launch application: {}", app_name);

        for &command in commands {
            debug!("Trying to launch with command: {}", command);
            
            let command_clone = command.to_string();
            let result = tokio::task::spawn_blocking(move || {
                Command::new(&command_clone)
                    .spawn()
            }).await.map_err(|e| {
                AutomationError::ApplicationFailed(format!("Failed to execute launch command: {e}"))
            })?;

            match result {
                Ok(mut child) => {
                    info!("Successfully launched {} with command: {}", app_name, command);
                    
                    // Don't wait for the process to complete, just ensure it started
                    tokio::task::spawn_blocking(move || {
                        let _ = child.wait();
                    });
                    
                    // Give the application a moment to start
                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                    return Ok(());
                }
                Err(e) => {
                    debug!("Failed to launch {} with command {}: {}", app_name, command, e);
                    continue;
                }
            }
        }

        Err(AutomationError::ApplicationFailed(format!(
            "Failed to launch {app_name} with any of the provided commands"
        )))
    }

    /// Minimize all windows to show desktop
    async fn minimize_all_windows(&self) -> Result<(), AutomationError> {
        debug!("Attempting to minimize all windows");

        // Try using wmctrl to minimize all windows
        let output = tokio::task::spawn_blocking(|| {
            Command::new("wmctrl")
                .args(["-k", "on"])
                .output()
        }).await.map_err(|e| {
            AutomationError::ApplicationFailed(format!("Failed to execute wmctrl minimize command: {e}"))
        })?;

        match output {
            Ok(output) if output.status.success() => {
                info!("Successfully minimized all windows using wmctrl");
                Ok(())
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(AutomationError::ApplicationFailed(format!(
                    "wmctrl minimize failed: {stderr}"
                )))
            }
            Err(e) => Err(AutomationError::ApplicationFailed(format!(
                "Failed to minimize windows: {e}"
            )))
        }
    }

    /// Send desktop shortcut key combination
    async fn send_desktop_shortcut(&self) -> Result<(), AutomationError> {
        debug!("Sending desktop shortcut key combination");

        // Try common desktop shortcuts (Super+D, Ctrl+Alt+D)
        let shortcuts = [
            vec!["Super_L", "d"],
            vec!["ctrl", "alt", "d"],
            vec!["Super_L", "m"], // Some DEs use Super+M to minimize all
        ];

        for shortcut in &shortcuts {
            debug!("Trying desktop shortcut: {:?}", shortcut);
            
            let shortcut_clone = shortcut.clone();
            let shortcut_str = shortcut.join("+");
            let result = tokio::task::spawn_blocking(move || {
                Command::new("xdotool")
                    .args(["key", &shortcut_clone.join("+")])
                    .output()
            }).await.map_err(|e| {
                AutomationError::ApplicationFailed(format!("Failed to execute xdotool shortcut command: {e}"))
            })?;

            match result {
                Ok(output) if output.status.success() => {
                    info!("Successfully sent desktop shortcut: {}", shortcut_str);
                    return Ok(());
                }
                Ok(_) => {
                    debug!("Desktop shortcut {} failed, trying next", shortcut_str);
                    continue;
                }
                Err(e) => {
                    debug!("Failed to send shortcut {}: {}", shortcut_str, e);
                    continue;
                }
            }
        }

        Err(AutomationError::ApplicationFailed(
            "Failed to send desktop shortcut with any key combination".to_string()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_application_service_creation() {
        let result = ApplicationService::new();
        assert!(
            result.is_ok(),
            "Application service should initialize successfully"
        );
    }

    #[tokio::test]
    async fn test_unsupported_application() {
        let service = ApplicationService::new().expect("Failed to create application service");
        let result = service.switch_to("unknown_app").await;
        assert!(result.is_err(), "Unknown application should return error");
        
        // Verify it's the correct error type
        match result.unwrap_err() {
            AutomationError::ApplicationFailed(msg) => {
                assert!(msg.contains("unknown_app"));
                assert!(msg.contains("not supported"));
            }
            _ => panic!("Expected ApplicationFailed error"),
        }
    }

    #[tokio::test]
    async fn test_supported_applications() {
        let service = ApplicationService::new().expect("Failed to create application service");
        
        // Test that supported applications are recognized (they may fail to launch in test environment)
        let supported_apps = [
            "firefox", "vscode", "code", "terminal", "desktop", 
            "directory", "files", "1password", "thunderbird"
        ];
        
        for app in &supported_apps {
            let result = service.switch_to(app).await;
            // In test environment, these will likely fail due to missing applications
            // but they should not return "not supported" error
            if let Err(AutomationError::ApplicationFailed(msg)) = &result {
                assert!(!msg.contains("not supported"), 
                    "Application {app} should be supported but got: {msg}");
            }
        }
    }

    #[tokio::test]
    async fn test_focus_existing_window_no_wmctrl() {
        let service = ApplicationService::new().expect("Failed to create application service");
        
        // This test will likely fail in most environments, but should not panic
        let result = service.focus_existing_window("nonexistent").await;
        assert!(result.is_err(), "Should fail to focus nonexistent window");
    }

    #[tokio::test]
    async fn test_launch_application_invalid_commands() {
        let service = ApplicationService::new().expect("Failed to create application service");
        
        // Test with commands that definitely don't exist
        let result = service.launch_application("test", &["nonexistent_command_12345"]).await;
        assert!(result.is_err(), "Should fail to launch with invalid command");
        
        match result.unwrap_err() {
            AutomationError::ApplicationFailed(msg) => {
                assert!(msg.contains("Failed to launch"));
            }
            _ => panic!("Expected ApplicationFailed error"),
        }
    }

    #[tokio::test]
    async fn test_minimize_all_windows() {
        let service = ApplicationService::new().expect("Failed to create application service");
        
        // This will likely fail in test environment without wmctrl, but should not panic
        let result = service.minimize_all_windows().await;
        // We don't assert success/failure as it depends on the test environment
        match result {
            Ok(()) => println!("Successfully minimized windows"),
            Err(e) => println!("Failed to minimize windows (expected in test env): {e}"),
        }
    }

    #[tokio::test]
    async fn test_send_desktop_shortcut() {
        let service = ApplicationService::new().expect("Failed to create application service");
        
        // This will likely fail in test environment without xdotool, but should not panic
        let result = service.send_desktop_shortcut().await;
        // We don't assert success/failure as it depends on the test environment
        match result {
            Ok(()) => println!("Successfully sent desktop shortcut"),
            Err(e) => println!("Failed to send desktop shortcut (expected in test env): {e}"),
        }
    }
}
