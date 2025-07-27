use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// User entity matching Prisma schema (Better Auth)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Validate)]
pub struct User {
    pub id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(length(
        min = 1,
        max = 255,
        message = "Name must be between 1 and 255 characters"
    ))]
    pub name: Option<String>,

    #[validate(email(message = "Invalid email format"))]
    pub email: String,

    #[serde(rename = "emailVerified")]
    pub email_verified: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,

    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Create a new user with default values
    pub fn new(email: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name: None,
            email,
            email_verified: false,
            image: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Get display name (name or email)
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.email)
    }

    /// Check if user has completed profile setup
    pub fn is_profile_complete(&self) -> bool {
        self.name.is_some() && self.email_verified
    }
}

/// Session entity matching Prisma schema (Better Auth)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Validate)]
pub struct Session {
    pub id: String,

    #[serde(rename = "userId")]
    pub user_id: String,

    pub token: String,

    #[serde(rename = "expiresAt")]
    pub expires_at: DateTime<Utc>,

    #[serde(rename = "ipAddress")]
    pub ip_address: Option<String>,

    #[serde(rename = "userAgent")]
    pub user_agent: Option<String>,

    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,

    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

impl Session {
    /// Create a new session
    pub fn new(user_id: String, token: String, expires_at: DateTime<Utc>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            token,
            expires_at,
            ip_address: None,
            user_agent: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if session is valid (not expired)
    pub fn is_valid(&self) -> bool {
        !self.is_expired()
    }

    /// Get remaining session duration in seconds
    pub fn remaining_seconds(&self) -> i64 {
        (self.expires_at - Utc::now()).num_seconds().max(0)
    }
}

/// Account entity matching Prisma schema (Better Auth)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Account {
    pub id: String,

    #[serde(rename = "userId")]
    pub user_id: String,

    #[serde(rename = "accountId")]
    pub account_id: String,

    #[serde(rename = "providerId")]
    pub provider_id: String,

    #[serde(rename = "accessToken")]
    pub access_token: Option<String>,

    #[serde(rename = "refreshToken")]
    pub refresh_token: Option<String>,

    #[serde(rename = "accessTokenExpiresAt")]
    pub access_token_expires_at: Option<DateTime<Utc>>,

    #[serde(rename = "refreshTokenExpiresAt")]
    pub refresh_token_expires_at: Option<DateTime<Utc>>,

    pub scope: Option<String>,

    #[serde(rename = "idToken")]
    pub id_token: Option<String>,

    pub password: Option<String>,

    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,

    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

impl Account {
    /// Create a new account
    pub fn new(user_id: String, account_id: String, provider_id: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            account_id,
            provider_id,
            access_token: None,
            refresh_token: None,
            access_token_expires_at: None,
            refresh_token_expires_at: None,
            scope: None,
            id_token: None,
            password: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if access token is expired
    pub fn is_access_token_expired(&self) -> bool {
        self.access_token_expires_at
            .map(|expires| Utc::now() > expires)
            .unwrap_or(false)
    }

    /// Check if refresh token is expired
    pub fn is_refresh_token_expired(&self) -> bool {
        self.refresh_token_expires_at
            .map(|expires| Utc::now() > expires)
            .unwrap_or(false)
    }

    /// Check if account needs token refresh
    pub fn needs_refresh(&self) -> bool {
        self.access_token.is_some()
            && self.refresh_token.is_some()
            && self.is_access_token_expired()
            && !self.is_refresh_token_expired()
    }
}

/// Verification entity matching Prisma schema (Better Auth)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Verification {
    pub id: String,
    pub identifier: String,
    pub value: String,

    #[serde(rename = "expiresAt")]
    pub expires_at: DateTime<Utc>,

    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,

    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

impl Verification {
    /// Create a new verification
    pub fn new(identifier: String, value: String, expires_at: DateTime<Utc>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            identifier,
            value,
            expires_at,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if verification is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if verification is valid (not expired)
    pub fn is_valid(&self) -> bool {
        !self.is_expired()
    }
}

/// File entity matching Prisma schema
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Validate)]
pub struct File {
    pub id: String,

    #[validate(length(
        min = 1,
        max = 255,
        message = "File name must be between 1 and 255 characters"
    ))]
    pub name: String,

    #[serde(rename = "type")]
    #[validate(length(
        min = 1,
        max = 100,
        message = "File type must be between 1 and 100 characters"
    ))]
    pub file_type: String, // MIME type

    #[validate(range(
        min = 0,
        max = 104857600,
        message = "File size must be between 0 and 100MB"
    ))] // 100MB limit
    pub size: i32, // Size in bytes

    pub data: String, // Base64 encoded file data

    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,

    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,

    #[serde(rename = "taskId")]
    pub task_id: String,
}

impl File {
    /// Create a new file
    pub fn new(name: String, file_type: String, size: i32, data: String, task_id: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            file_type,
            size,
            data,
            created_at: now,
            updated_at: now,
            task_id,
        }
    }

    /// Get file extension from name
    pub fn extension(&self) -> Option<&str> {
        self.name.split('.').next_back()
    }

    /// Check if file is an image
    pub fn is_image(&self) -> bool {
        self.file_type.starts_with("image/")
    }

    /// Check if file is a document
    pub fn is_document(&self) -> bool {
        matches!(
            self.file_type.as_str(),
            "application/pdf"
                | "application/msword"
                | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                | "text/plain"
                | "text/markdown"
        )
    }

    /// Get human-readable file size
    pub fn human_readable_size(&self) -> String {
        let size = self.size as f64;
        if size < 1024.0 {
            format!("{size} B")
        } else if size < 1024.0 * 1024.0 {
            format!("{:.1} KB", size / 1024.0)
        } else if size < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.1} MB", size / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", size / (1024.0 * 1024.0 * 1024.0))
        }
    }

    /// Validate file data integrity
    pub fn validate_data(&self) -> Result<(), String> {
        // Validate base64 encoding
        use base64::{engine::general_purpose, Engine as _};
        match general_purpose::STANDARD.decode(&self.data) {
            Ok(decoded) => {
                if decoded.len() != self.size as usize {
                    Err("File size doesn't match decoded data length".to_string())
                } else {
                    Ok(())
                }
            }
            Err(_) => Err("Invalid base64 encoding in file data".to_string()),
        }
    }
}
