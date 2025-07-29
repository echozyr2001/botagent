use std::path::{Path, PathBuf};

use base64::{engine::general_purpose, Engine as _};
use tokio::fs;
use tracing::{debug, error, warn};

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

    /// Write base64 encoded data to file
    pub async fn write_file(&self, path: &str, data: &str) -> Result<(), AutomationError> {
        debug!("Writing file: {}", path);

        let path = self.validate_and_normalize_path(path)?;

        // Decode base64 data
        let content = general_purpose::STANDARD.decode(data).map_err(|e| {
            error!("Failed to decode base64 data: {}", e);
            AutomationError::FileFailed(format!("Invalid base64 data: {e}"))
        })?;

        // Check decoded content size
        let content_size_mb = content.len() as u64 / (1024 * 1024);
        if content_size_mb > self.max_file_size_mb {
            return Err(AutomationError::FileTooLarge {
                size: content_size_mb,
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

        // Write file content
        fs::write(&path, &content).await.map_err(|e| {
            error!("Failed to write file {}: {}", path.display(), e);
            AutomationError::FileFailed(format!("Failed to write file: {e}"))
        })?;

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

    fn validate_and_normalize_path(&self, path: &str) -> Result<PathBuf, AutomationError> {
        if path.is_empty() {
            return Err(AutomationError::InvalidPath("Empty path".to_string()));
        }

        // Basic path traversal protection
        if path.contains("..") {
            warn!("Path traversal attempt detected: {}", path);
            return Err(AutomationError::InvalidPath(
                "Path traversal not allowed".to_string(),
            ));
        }

        // Convert to absolute path
        let path = Path::new(path);
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| {
                    AutomationError::FileFailed(format!("Failed to get current directory: {e}"))
                })?
                .join(path)
        };

        // Normalize the path
        let normalized = absolute_path.canonicalize().unwrap_or(absolute_path);

        Ok(normalized)
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
    use tempfile::NamedTempFile;

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

        // Create a temporary file
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let temp_path = temp_file.path().to_str().unwrap();

        // Write some text
        let test_text = "Hello, World!";
        let result = service.write_text_file(temp_path, test_text).await;
        assert!(result.is_ok(), "Writing text file should succeed");

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
    }
}
