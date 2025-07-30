use std::path::{Path, PathBuf};

use base64::{engine::general_purpose, Engine as _};
use tokio::fs;
use tracing::{debug, error, info, warn};

use crate::error::AutomationError;

#[derive(Debug, Clone)]
pub struct FileService {
    max_file_size_mb: u64,
}

impl FileService {
    pub fn new() -> Result<Self, AutomationError> {
        Ok(Self {
            max_file_size_mb: 10, // Default 10MB limit
        })
    }

    pub fn with_max_size(max_size_mb: u64) -> Result<Self, AutomationError> {
        if max_size_mb == 0 {
            return Err(AutomationError::Validation(
                "Maximum file size must be greater than 0".to_string(),
            ));
        }
        if max_size_mb > 1024 {
            return Err(AutomationError::Validation(
                "Maximum file size cannot exceed 1024 MB".to_string(),
            ));
        }
        Ok(Self {
            max_file_size_mb: max_size_mb,
        })
    }

    /// Read file content and return as base64 encoded string
    pub async fn read_file(&self, path: &str) -> Result<String, AutomationError> {
        debug!("Reading file: {}", path);

        let path = self.validate_and_normalize_path(path)?;

        // Check if file exists
        if !path.exists() {
            return Err(AutomationError::FileFailed(format!(
                "File does not exist: {}",
                path.display()
            )));
        }

        // Check if it's a file (not a directory)
        if !path.is_file() {
            return Err(AutomationError::FileFailed(format!(
                "Path is not a file: {}",
                path.display()
            )));
        }

        // Check file size
        let metadata = fs::metadata(&path).await.map_err(|e| {
            error!("Failed to get file metadata for {}: {}", path.display(), e);
            AutomationError::FileFailed(format!("Failed to get file metadata: {e}"))
        })?;

        let file_size_mb = metadata.len() / (1024 * 1024);
        if file_size_mb > self.max_file_size_mb {
            return Err(AutomationError::FileTooLarge {
                size: file_size_mb,
                limit: self.max_file_size_mb,
            });
        }

        // Read file content
        let content = fs::read(&path).await.map_err(|e| {
            error!("Failed to read file {}: {}", path.display(), e);
            AutomationError::FileFailed(format!("Failed to read file: {e}"))
        })?;

        // Encode as base64
        let base64_content = general_purpose::STANDARD.encode(&content);
        Ok(base64_content)
    }

    /// Write base64 encoded data to file with enhanced validation
    pub async fn write_file(&self, path: &str, data: &str) -> Result<(), AutomationError> {
        debug!("Writing file: {} ({} bytes base64)", path, data.len());

        let path = self.validate_and_normalize_path(path)?;

        // Validate base64 data format first
        if data.is_empty() {
            return Err(AutomationError::Validation(
                "Cannot write empty base64 data".to_string(),
            ));
        }

        // Decode base64 data
        let content = general_purpose::STANDARD.decode(data).map_err(|e| {
            error!("Failed to decode base64 data: {}", e);
            AutomationError::Validation(format!("Invalid base64 data: {e}"))
        })?;

        // Check decoded content size
        let content_size_mb = content.len() as u64 / (1024 * 1024);
        if content_size_mb > self.max_file_size_mb {
            warn!(
                "File size {} MB exceeds limit of {} MB",
                content_size_mb, self.max_file_size_mb
            );
            return Err(AutomationError::FileTooLarge {
                size: content_size_mb,
                limit: self.max_file_size_mb,
            });
        }

        // Additional content validation
        self.validate_file_content(&content, &path)?;

        // Check if file already exists and get backup if needed
        let backup_needed = path.exists();
        let backup_path = if backup_needed {
            Some(path.with_extension(format!(
                "{}.backup",
                path.extension().and_then(|e| e.to_str()).unwrap_or("tmp")
            )))
        } else {
            None
        };

        // Create backup if file exists
        if let Some(backup) = &backup_path {
            if let Err(e) = fs::copy(&path, backup).await {
                warn!("Failed to create backup for {}: {}", path.display(), e);
                // Continue without backup - not critical
            } else {
                debug!("Created backup at: {}", backup.display());
            }
        }

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                error!(
                    "Failed to create parent directories for {}: {}",
                    path.display(),
                    e
                );
                AutomationError::FileFailed(format!("Failed to create directories: {e}"))
            })?;
        }

        // Write file content
        match fs::write(&path, &content).await {
            Ok(()) => {
                info!(
                    "Successfully wrote file: {} ({} bytes)",
                    path.display(),
                    content.len()
                );

                // Clean up backup if write was successful
                if let Some(backup) = backup_path {
                    let _ = fs::remove_file(backup).await; // Ignore errors for cleanup
                }

                Ok(())
            }
            Err(e) => {
                error!("Failed to write file {}: {}", path.display(), e);

                // Restore from backup if available
                if let Some(backup) = backup_path {
                    if backup.exists() {
                        if let Err(restore_err) = fs::copy(&backup, &path).await {
                            error!("Failed to restore backup: {}", restore_err);
                        } else {
                            info!("Restored file from backup");
                        }
                        let _ = fs::remove_file(backup).await; // Clean up backup
                    }
                }

                Err(AutomationError::FileFailed(format!(
                    "Failed to write file: {e}"
                )))
            }
        }
    }

    /// Validate file content for security and safety
    fn validate_file_content(&self, content: &[u8], path: &Path) -> Result<(), AutomationError> {
        // Check for executable file extensions and warn
        if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
            let executable_extensions = [
                "exe", "bat", "cmd", "com", "scr", "pif", "vbs", "js", "jar", "sh", "bash", "zsh",
                "fish", "py", "pl", "rb", "php",
            ];

            if executable_extensions
                .iter()
                .any(|&ext| ext.eq_ignore_ascii_case(extension))
            {
                warn!("Writing potentially executable file: {}", path.display());

                // Additional check for script content
                if let Ok(text_content) = std::str::from_utf8(content) {
                    let dangerous_patterns = [
                        "#!/bin/",
                        "#!/usr/bin/",
                        "@echo off",
                        "powershell",
                        "cmd.exe",
                        "system(",
                        "exec(",
                        "eval(",
                        "subprocess",
                    ];

                    for pattern in &dangerous_patterns {
                        if text_content
                            .to_lowercase()
                            .contains(&pattern.to_lowercase())
                        {
                            return Err(AutomationError::Validation(
                                "File contains potentially dangerous executable content"
                                    .to_string(),
                            ));
                        }
                    }
                }
            }
        }

        // Check for binary content that might be malicious
        if content.len() > 1024 {
            // Look for PE header (Windows executable)
            if content.len() > 64 && &content[0..2] == b"MZ" {
                return Err(AutomationError::Validation(
                    "Cannot write Windows executable files".to_string(),
                ));
            }

            // Look for ELF header (Linux executable)
            if content.len() > 4 && &content[0..4] == b"\x7fELF" {
                return Err(AutomationError::Validation(
                    "Cannot write Linux executable files".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Write plain text to file
    pub async fn write_text_file(&self, path: &str, text: &str) -> Result<(), AutomationError> {
        debug!("Writing text file: {}", path);

        let path = self.validate_and_normalize_path(path)?;

        // Check text size
        let text_size_mb = text.len() as u64 / (1024 * 1024);
        if text_size_mb > self.max_file_size_mb {
            return Err(AutomationError::FileTooLarge {
                size: text_size_mb,
                limit: self.max_file_size_mb,
            });
        }

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                error!(
                    "Failed to create parent directories for {}: {}",
                    path.display(),
                    e
                );
                AutomationError::FileFailed(format!("Failed to create directories: {e}"))
            })?;
        }

        // Write text content
        fs::write(&path, text).await.map_err(|e| {
            error!("Failed to write text file {}: {}", path.display(), e);
            AutomationError::FileFailed(format!("Failed to write text file: {e}"))
        })?;

        Ok(())
    }

    /// Read plain text from file
    pub async fn read_text_file(&self, path: &str) -> Result<String, AutomationError> {
        debug!("Reading text file: {}", path);

        let path = self.validate_and_normalize_path(path)?;

        // Check if file exists
        if !path.exists() {
            return Err(AutomationError::FileFailed(format!(
                "File does not exist: {}",
                path.display()
            )));
        }

        // Check if it's a file (not a directory)
        if !path.is_file() {
            return Err(AutomationError::FileFailed(format!(
                "Path is not a file: {}",
                path.display()
            )));
        }

        // Check file size
        let metadata = fs::metadata(&path).await.map_err(|e| {
            error!("Failed to get file metadata for {}: {}", path.display(), e);
            AutomationError::FileFailed(format!("Failed to get file metadata: {e}"))
        })?;

        let file_size_mb = metadata.len() / (1024 * 1024);
        if file_size_mb > self.max_file_size_mb {
            return Err(AutomationError::FileTooLarge {
                size: file_size_mb,
                limit: self.max_file_size_mb,
            });
        }

        // Read file content as text
        let content = fs::read_to_string(&path).await.map_err(|e| {
            error!("Failed to read text file {}: {}", path.display(), e);
            AutomationError::FileFailed(format!("Failed to read text file: {e}"))
        })?;

        Ok(content)
    }

    /// Check if file exists
    pub async fn file_exists(&self, path: &str) -> Result<bool, AutomationError> {
        let path = self.validate_and_normalize_path(path)?;
        Ok(path.exists() && path.is_file())
    }

    /// Get file information
    pub async fn get_file_info(&self, path: &str) -> Result<FileInfo, AutomationError> {
        let path = self.validate_and_normalize_path(path)?;

        if !path.exists() {
            return Err(AutomationError::FileFailed(format!(
                "File does not exist: {}",
                path.display()
            )));
        }

        let metadata = fs::metadata(&path).await.map_err(|e| {
            error!("Failed to get file metadata for {}: {}", path.display(), e);
            AutomationError::FileFailed(format!("Failed to get file metadata: {e}"))
        })?;

        Ok(FileInfo {
            path: path.to_string_lossy().to_string(),
            size: metadata.len(),
            is_file: metadata.is_file(),
            is_directory: metadata.is_dir(),
            modified: metadata.modified().ok(),
        })
    }

    /// Delete a file with additional safety checks
    pub async fn delete_file(&self, path: &str) -> Result<(), AutomationError> {
        debug!("Deleting file: {}", path);

        let path = self.validate_and_normalize_path(path)?;

        if !path.exists() {
            return Err(AutomationError::FileFailed(format!(
                "File does not exist: {}",
                path.display()
            )));
        }

        if !path.is_file() {
            return Err(AutomationError::FileFailed(format!(
                "Path is not a file: {}",
                path.display()
            )));
        }

        // Additional safety check: prevent deletion of important files
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let protected_files = [
            ".env",
            ".env.local",
            ".env.production",
            "package.json",
            "Cargo.toml",
            "requirements.txt",
            "docker-compose.yml",
            "Dockerfile",
            ".gitignore",
            ".git",
            "README.md",
        ];

        if protected_files.contains(&file_name) {
            warn!("Attempt to delete protected file: {}", path.display());
            return Err(AutomationError::InvalidPath(
                "Cannot delete protected system files".to_string(),
            ));
        }

        fs::remove_file(&path).await.map_err(|e| {
            error!("Failed to delete file {}: {}", path.display(), e);
            AutomationError::FileFailed(format!("Failed to delete file: {e}"))
        })?;

        info!("Successfully deleted file: {}", path.display());
        Ok(())
    }

    /// Create a directory
    pub async fn create_directory(&self, path: &str) -> Result<(), AutomationError> {
        debug!("Creating directory: {}", path);

        let path = self.validate_and_normalize_path(path)?;

        fs::create_dir_all(&path).await.map_err(|e| {
            error!("Failed to create directory {}: {}", path.display(), e);
            AutomationError::FileFailed(format!("Failed to create directory: {e}"))
        })?;

        Ok(())
    }

    /// List directory contents
    pub async fn list_directory(&self, path: &str) -> Result<Vec<FileInfo>, AutomationError> {
        debug!("Listing directory: {}", path);

        let path = self.validate_and_normalize_path(path)?;

        if !path.exists() {
            return Err(AutomationError::FileFailed(format!(
                "Directory does not exist: {}",
                path.display()
            )));
        }

        if !path.is_dir() {
            return Err(AutomationError::FileFailed(format!(
                "Path is not a directory: {}",
                path.display()
            )));
        }

        let mut entries = fs::read_dir(&path).await.map_err(|e| {
            error!("Failed to read directory {}: {}", path.display(), e);
            AutomationError::FileFailed(format!("Failed to read directory: {e}"))
        })?;

        let mut file_infos = Vec::new();

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            error!("Failed to read directory entry: {}", e);
            AutomationError::FileFailed(format!("Failed to read directory entry: {e}"))
        })? {
            let entry_path = entry.path();
            let metadata = entry.metadata().await.map_err(|e| {
                error!("Failed to get metadata for {}: {}", entry_path.display(), e);
                AutomationError::FileFailed(format!("Failed to get entry metadata: {e}"))
            })?;

            file_infos.push(FileInfo {
                path: entry_path.to_string_lossy().to_string(),
                size: metadata.len(),
                is_file: metadata.is_file(),
                is_directory: metadata.is_dir(),
                modified: metadata.modified().ok(),
            });
        }

        Ok(file_infos)
    }

    fn validate_and_normalize_path(&self, path: &str) -> Result<PathBuf, AutomationError> {
        // Basic validation
        if path.is_empty() {
            return Err(AutomationError::InvalidPath("Empty path".to_string()));
        }

        // Check path length to prevent extremely long paths
        if path.len() > 4096 {
            return Err(AutomationError::InvalidPath(
                "Path too long (maximum 4096 characters)".to_string(),
            ));
        }

        // Enhanced path traversal protection
        if path.contains("..") || path.contains("~") {
            warn!("Path traversal attempt detected: {}", path);
            return Err(AutomationError::InvalidPath(
                "Path traversal not allowed".to_string(),
            ));
        }

        // Check for null bytes and other dangerous characters
        if path.contains('\0') {
            return Err(AutomationError::InvalidPath(
                "Null bytes not allowed in path".to_string(),
            ));
        }

        // Check for other dangerous characters that could be used for injection
        let dangerous_chars = ['<', '>', '|', '"', '*', '?'];
        for &ch in &dangerous_chars {
            if path.contains(ch) {
                return Err(AutomationError::InvalidPath(format!(
                    "Dangerous character '{ch}' not allowed in path"
                )));
            }
        }

        // Prevent access to sensitive system directories
        let dangerous_prefixes = [
            "/etc/",
            "/proc/",
            "/sys/",
            "/dev/",
            "/boot/",
            "/var/run/",
            "/root/",
            "/var/log/",
            "/usr/bin/",
            "/usr/sbin/",
            "/sbin/",
            "/lib/",
            "/lib64/",
            "/usr/lib/",
            "/usr/lib64/",
            "C:\\Windows\\",
            "C:\\Program Files\\",
            "C:\\Program Files (x86)\\",
            "C:\\Users\\Administrator\\",
            "C:\\System Volume Information\\",
            "C:\\$Recycle.Bin\\",
            "C:\\ProgramData\\Microsoft\\",
        ];

        let normalized_path = path.to_lowercase();
        for prefix in &dangerous_prefixes {
            if normalized_path.starts_with(&prefix.to_lowercase()) {
                warn!("Attempt to access restricted directory: {}", path);
                return Err(AutomationError::InvalidPath(
                    "Access to system directories not allowed".to_string(),
                ));
            }
        }

        // Additional Windows-specific checks
        #[cfg(target_os = "windows")]
        {
            // Check for Windows reserved names
            let reserved_names = [
                "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
                "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
                "LPT9",
            ];

            let path_upper = path.to_uppercase();
            for reserved in &reserved_names {
                if path_upper.contains(reserved) {
                    return Err(AutomationError::InvalidPath(format!(
                        "Reserved Windows name '{}' not allowed in path",
                        reserved
                    )));
                }
            }
        }

        // Convert to absolute path
        let path_buf = Path::new(path);
        let absolute_path = if path_buf.is_absolute() {
            path_buf.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| {
                    error!("Failed to get current directory: {}", e);
                    AutomationError::FileFailed(format!("Failed to get current directory: {e}"))
                })?
                .join(path_buf)
        };

        // Normalize the path and ensure it doesn't escape the working directory
        let normalized = match absolute_path.canonicalize() {
            Ok(canonical) => canonical,
            Err(_) => {
                // If canonicalize fails (e.g., file doesn't exist), validate the parent directory
                if let Some(parent) = absolute_path.parent() {
                    if parent.exists() {
                        // Validate that the parent is accessible
                        match parent.canonicalize() {
                            Ok(canonical_parent) => {
                                // Ensure the parent is within allowed boundaries
                                if !self.is_path_allowed(&canonical_parent)? {
                                    return Err(AutomationError::InvalidPath(
                                        "Parent directory outside allowed boundaries".to_string(),
                                    ));
                                }
                                absolute_path
                            }
                            Err(e) => {
                                error!("Failed to canonicalize parent directory: {}", e);
                                return Err(AutomationError::InvalidPath(
                                    "Invalid parent directory".to_string(),
                                ));
                            }
                        }
                    } else {
                        return Err(AutomationError::InvalidPath(
                            "Parent directory does not exist".to_string(),
                        ));
                    }
                } else {
                    return Err(AutomationError::InvalidPath(
                        "Invalid path structure".to_string(),
                    ));
                }
            }
        };

        // Final security check: ensure the normalized path is within allowed boundaries
        if !self.is_path_allowed(&normalized)? {
            warn!(
                "Attempt to access file outside allowed directories: {}",
                normalized.display()
            );
            return Err(AutomationError::InvalidPath(
                "Access outside allowed directories not permitted".to_string(),
            ));
        }

        Ok(normalized)
    }

    /// Check if a path is within allowed boundaries
    fn is_path_allowed(&self, path: &Path) -> Result<bool, AutomationError> {
        let current_dir = std::env::current_dir().map_err(|e| {
            error!("Failed to get current directory: {}", e);
            AutomationError::FileFailed(format!("Failed to get current directory: {e}"))
        })?;

        // Allow access to current directory and subdirectories, plus common user directories
        let allowed_prefixes = [
            current_dir.clone(),
            dirs::home_dir().unwrap_or_else(|| current_dir.clone()),
            dirs::desktop_dir().unwrap_or_else(|| current_dir.clone()),
            dirs::document_dir().unwrap_or_else(|| current_dir.clone()),
            dirs::download_dir().unwrap_or_else(|| current_dir.clone()),
            dirs::picture_dir().unwrap_or_else(|| current_dir.clone()),
            dirs::video_dir().unwrap_or_else(|| current_dir.clone()),
            dirs::audio_dir().unwrap_or_else(|| current_dir.clone()),
            std::env::temp_dir(),
        ];

        // Check if the path starts with any allowed prefix
        let is_allowed = allowed_prefixes
            .iter()
            .any(|allowed| path.starts_with(allowed));

        // Additional check: allow paths explicitly configured via environment variable
        if !is_allowed {
            if let Ok(additional_paths) = std::env::var("BYTEBOT_ALLOWED_PATHS") {
                let additional_allowed: Vec<PathBuf> = additional_paths
                    .split(':')
                    .filter_map(|p| {
                        if !p.is_empty() {
                            Some(PathBuf::from(p))
                        } else {
                            None
                        }
                    })
                    .collect();

                return Ok(additional_allowed
                    .iter()
                    .any(|allowed| path.starts_with(allowed)));
            }
        }

        Ok(is_allowed)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
    pub is_file: bool,
    pub is_directory: bool,
    pub modified: Option<std::time::SystemTime>,
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_file_service_creation() {
        let result = FileService::new();
        assert!(
            result.is_ok(),
            "File service should initialize successfully"
        );
    }

    #[tokio::test]
    async fn test_validate_path() {
        let service = FileService::new().expect("Failed to create file service");

        // Valid path
        let result = service.validate_and_normalize_path("test.txt");
        assert!(result.is_ok(), "Valid path should be accepted");

        // Path traversal attempt
        let result = service.validate_and_normalize_path("../test.txt");
        assert!(result.is_err(), "Path traversal should be rejected");

        // Empty path
        let result = service.validate_and_normalize_path("");
        assert!(result.is_err(), "Empty path should be rejected");
    }

    #[tokio::test]
    async fn test_file_operations() {
        let service = FileService::new().expect("Failed to create file service");

        // Create a temporary file in the current directory (which should be allowed)
        let temp_path = "./test_temp_file.txt";

        // Write some text
        let test_text = "Hello, World!";
        let result = service.write_text_file(temp_path, test_text).await;
        assert!(
            result.is_ok(),
            "Writing text file should succeed: {:?}",
            result.err()
        );

        // Read the text back
        let result = service.read_text_file(temp_path).await;
        assert!(result.is_ok(), "Reading text file should succeed");
        assert_eq!(
            result.unwrap(),
            test_text,
            "Read text should match written text"
        );

        // Check if file exists
        let exists = service.file_exists(temp_path).await.unwrap();
        assert!(exists, "File should exist");

        // Get file info
        let info = service.get_file_info(temp_path).await;
        assert!(info.is_ok(), "Getting file info should succeed");
        let info = info.unwrap();
        assert!(info.is_file, "Should be identified as a file");
        assert!(
            !info.is_directory,
            "Should not be identified as a directory"
        );

        // Clean up
        let _ = tokio::fs::remove_file(temp_path).await;
    }
}
