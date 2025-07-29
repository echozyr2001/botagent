#[cfg(test)]
mod tests {
    use bytebot_shared_rs::types::api::PaginationParams;
    use chrono::{Duration, Utc};

    use crate::database::{
        tests::{cleanup_test_data, create_test_pool},
        user_repository::{
            CreateAccountDto, CreateSessionDto, CreateUserDto, UpdateUserDto, UserRepository,
            UserRepositoryTrait,
        },
        DatabaseError,
    };

    #[tokio::test]
    async fn test_create_user() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        let dto = CreateUserDto {
            email: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
            email_verified: Some(true),
            image: Some("https://example.com/avatar.jpg".to_string()),
        };

        let result = repo.create_user(&dto).await;
        assert!(result.is_ok());

        let user = result.unwrap();
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.name, Some("Test User".to_string()));
        assert!(user.email_verified);
        assert_eq!(
            user.image,
            Some("https://example.com/avatar.jpg".to_string())
        );

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_create_user_validation() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        // Test invalid email
        let dto = CreateUserDto {
            email: "invalid-email".to_string(),
            name: None,
            email_verified: None,
            image: None,
        };

        let result = repo.create_user(&dto).await;
        assert!(result.is_err());

        if let Err(DatabaseError::ValidationError(msg)) = result {
            assert!(msg.contains("Invalid email format"));
        } else {
            panic!("Expected ValidationError for invalid email");
        }

        // Test empty email
        let dto = CreateUserDto {
            email: "".to_string(),
            name: None,
            email_verified: None,
            image: None,
        };

        let result = repo.create_user(&dto).await;
        assert!(result.is_err());

        // Test empty name
        let dto = CreateUserDto {
            email: "test@example.com".to_string(),
            name: Some("".to_string()),
            email_verified: None,
            image: None,
        };

        let result = repo.create_user(&dto).await;
        assert!(result.is_err());

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_create_user_duplicate_email() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        let dto = CreateUserDto {
            email: "duplicate@example.com".to_string(),
            name: Some("First User".to_string()),
            email_verified: None,
            image: None,
        };

        // Create first user
        let result = repo.create_user(&dto).await;
        assert!(result.is_ok());

        // Try to create second user with same email
        let dto2 = CreateUserDto {
            email: "duplicate@example.com".to_string(),
            name: Some("Second User".to_string()),
            email_verified: None,
            image: None,
        };

        let result = repo.create_user(&dto2).await;
        assert!(result.is_err());

        if let Err(DatabaseError::ValidationError(msg)) = result {
            assert!(msg.contains("already exists"));
        } else {
            panic!("Expected ValidationError for duplicate email");
        }

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_get_user_by_id() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        // Create a user first
        let dto = CreateUserDto {
            email: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
            email_verified: Some(false),
            image: None,
        };

        let created_user = repo.create_user(&dto).await.expect("Failed to create user");

        // Test getting by ID
        let result = repo.get_user_by_id(&created_user.id).await;
        assert!(result.is_ok());

        let user = result.unwrap();
        assert!(user.is_some());

        let user = user.unwrap();
        assert_eq!(user.id, created_user.id);
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.name, Some("Test User".to_string()));
        assert!(!user.email_verified);

        // Test getting non-existent user
        let result = repo.get_user_by_id("non-existent-id").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_get_user_by_email() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        // Create a user first
        let dto = CreateUserDto {
            email: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
            email_verified: Some(true),
            image: None,
        };

        let created_user = repo.create_user(&dto).await.expect("Failed to create user");

        // Test getting by email
        let result = repo.get_user_by_email("test@example.com").await;
        assert!(result.is_ok());

        let user = result.unwrap();
        assert!(user.is_some());

        let user = user.unwrap();
        assert_eq!(user.id, created_user.id);
        assert_eq!(user.email, "test@example.com");

        // Test getting non-existent user
        let result = repo.get_user_by_email("nonexistent@example.com").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Test invalid email format
        let result = repo.get_user_by_email("invalid-email").await;
        assert!(result.is_err());

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_update_user() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        // Create a user first
        let dto = CreateUserDto {
            email: "original@example.com".to_string(),
            name: Some("Original Name".to_string()),
            email_verified: Some(false),
            image: None,
        };

        let created_user = repo.create_user(&dto).await.expect("Failed to create user");

        // Update the user
        let update_dto = UpdateUserDto {
            name: Some("Updated Name".to_string()),
            email: Some("updated@example.com".to_string()),
            email_verified: Some(true),
            image: Some("https://example.com/new-avatar.jpg".to_string()),
        };

        let result = repo.update_user(&created_user.id, &update_dto).await;
        assert!(result.is_ok());

        let updated_user = result.unwrap();
        assert!(updated_user.is_some());

        let updated_user = updated_user.unwrap();
        assert_eq!(updated_user.id, created_user.id);
        assert_eq!(updated_user.name, Some("Updated Name".to_string()));
        assert_eq!(updated_user.email, "updated@example.com");
        assert!(updated_user.email_verified);
        assert_eq!(
            updated_user.image,
            Some("https://example.com/new-avatar.jpg".to_string())
        );
        assert!(updated_user.updated_at > created_user.updated_at);

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_delete_user() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        // Create a user first
        let dto = CreateUserDto {
            email: "delete@example.com".to_string(),
            name: Some("User To Delete".to_string()),
            email_verified: None,
            image: None,
        };

        let created_user = repo.create_user(&dto).await.expect("Failed to create user");

        // Delete the user
        let result = repo.delete_user(&created_user.id).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Verify user is deleted
        let result = repo.get_user_by_id(&created_user.id).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Test deleting non-existent user
        let result = repo.delete_user("non-existent-id").await;
        assert!(result.is_ok());
        assert!(!result.unwrap());

        cleanup_test_data(&pool).await;
    }
    #[tokio::test]
    async fn test_verify_user_email() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        // Create a user with unverified email
        let dto = CreateUserDto {
            email: "unverified@example.com".to_string(),
            name: Some("Unverified User".to_string()),
            email_verified: Some(false),
            image: None,
        };

        let created_user = repo.create_user(&dto).await.expect("Failed to create user");
        assert!(!created_user.email_verified);

        // Verify the email
        let result = repo.verify_user_email(&created_user.id).await;
        assert!(result.is_ok());

        let verified_user = result.unwrap();
        assert!(verified_user.is_some());

        let verified_user = verified_user.unwrap();
        assert_eq!(verified_user.id, created_user.id);
        assert!(verified_user.email_verified);
        assert!(verified_user.updated_at > created_user.updated_at);

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_create_session() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        // Create a user first
        let user_dto = CreateUserDto {
            email: "session@example.com".to_string(),
            name: Some("Session User".to_string()),
            email_verified: Some(true),
            image: None,
        };

        let user = repo
            .create_user(&user_dto)
            .await
            .expect("Failed to create user");

        // Create a session
        let expires_at = Utc::now() + Duration::hours(24);
        let session_dto = CreateSessionDto {
            user_id: user.id.clone(),
            token: "test-session-token".to_string(),
            expires_at,
            ip_address: Some("192.168.1.1".to_string()),
            user_agent: Some("Test User Agent".to_string()),
        };

        let result = repo.create_session(&session_dto).await;
        assert!(result.is_ok());

        let session = result.unwrap();
        assert_eq!(session.user_id, user.id);
        assert_eq!(session.token, "test-session-token");
        assert_eq!(session.expires_at, expires_at);
        assert_eq!(session.ip_address, Some("192.168.1.1".to_string()));
        assert_eq!(session.user_agent, Some("Test User Agent".to_string()));

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_get_session_by_token() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        // Create a user first
        let user_dto = CreateUserDto {
            email: "session@example.com".to_string(),
            name: Some("Session User".to_string()),
            email_verified: Some(true),
            image: None,
        };

        let user = repo
            .create_user(&user_dto)
            .await
            .expect("Failed to create user");

        // Create a session
        let expires_at = Utc::now() + Duration::hours(24);
        let session_dto = CreateSessionDto {
            user_id: user.id,
            token: "unique-session-token".to_string(),
            expires_at,
            ip_address: None,
            user_agent: None,
        };

        let created_session = repo
            .create_session(&session_dto)
            .await
            .expect("Failed to create session");

        // Test getting by token
        let result = repo.get_session_by_token("unique-session-token").await;
        assert!(result.is_ok());

        let session = result.unwrap();
        assert!(session.is_some());

        let session = session.unwrap();
        assert_eq!(session.id, created_session.id);
        assert_eq!(session.token, "unique-session-token");

        // Test getting non-existent session
        let result = repo.get_session_by_token("non-existent-token").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_delete_expired_sessions() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        // Create a user first
        let user_dto = CreateUserDto {
            email: "session@example.com".to_string(),
            name: Some("Session User".to_string()),
            email_verified: Some(true),
            image: None,
        };

        let user = repo
            .create_user(&user_dto)
            .await
            .expect("Failed to create user");

        // Create expired session
        let expired_session_dto = CreateSessionDto {
            user_id: user.id.clone(),
            token: "expired-token".to_string(),
            expires_at: Utc::now() - Duration::hours(1), // Expired 1 hour ago
            ip_address: None,
            user_agent: None,
        };

        repo.create_session(&expired_session_dto)
            .await
            .expect("Failed to create expired session");

        // Create valid session
        let valid_session_dto = CreateSessionDto {
            user_id: user.id,
            token: "valid-token".to_string(),
            expires_at: Utc::now() + Duration::hours(24), // Valid for 24 hours
            ip_address: None,
            user_agent: None,
        };

        repo.create_session(&valid_session_dto)
            .await
            .expect("Failed to create valid session");

        // Delete expired sessions
        let result = repo.delete_expired_sessions().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // Should delete 1 expired session

        // Verify expired session is deleted
        let result = repo.get_session_by_token("expired-token").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Verify valid session still exists
        let result = repo.get_session_by_token("valid-token").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_create_account() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        // Create a user first
        let user_dto = CreateUserDto {
            email: "account@example.com".to_string(),
            name: Some("Account User".to_string()),
            email_verified: Some(true),
            image: None,
        };

        let user = repo
            .create_user(&user_dto)
            .await
            .expect("Failed to create user");

        // Create an account
        let account_dto = CreateAccountDto {
            user_id: user.id.clone(),
            account_id: "oauth-account-123".to_string(),
            provider_id: "google".to_string(),
            access_token: Some("access-token-123".to_string()),
            refresh_token: Some("refresh-token-123".to_string()),
            access_token_expires_at: Some(Utc::now() + Duration::hours(1)),
            refresh_token_expires_at: Some(Utc::now() + Duration::days(30)),
            scope: Some("read write".to_string()),
            id_token: Some("id-token-123".to_string()),
            password: None,
        };

        let result = repo.create_account(&account_dto).await;
        assert!(result.is_ok());

        let account = result.unwrap();
        assert_eq!(account.user_id, user.id);
        assert_eq!(account.account_id, "oauth-account-123");
        assert_eq!(account.provider_id, "google");
        assert_eq!(account.access_token, Some("access-token-123".to_string()));
        assert_eq!(account.refresh_token, Some("refresh-token-123".to_string()));
        assert_eq!(account.scope, Some("read write".to_string()));

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_get_account_by_provider() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        // Create a user first
        let user_dto = CreateUserDto {
            email: "account@example.com".to_string(),
            name: Some("Account User".to_string()),
            email_verified: Some(true),
            image: None,
        };

        let user = repo
            .create_user(&user_dto)
            .await
            .expect("Failed to create user");

        // Create an account
        let account_dto = CreateAccountDto {
            user_id: user.id.clone(),
            account_id: "github-account-456".to_string(),
            provider_id: "github".to_string(),
            access_token: Some("github-access-token".to_string()),
            refresh_token: None,
            access_token_expires_at: None,
            refresh_token_expires_at: None,
            scope: Some("repo user".to_string()),
            id_token: None,
            password: None,
        };

        let created_account = repo
            .create_account(&account_dto)
            .await
            .expect("Failed to create account");

        // Test getting by provider
        let result = repo.get_account_by_provider(&user.id, "github").await;
        assert!(result.is_ok());

        let account = result.unwrap();
        assert!(account.is_some());

        let account = account.unwrap();
        assert_eq!(account.id, created_account.id);
        assert_eq!(account.provider_id, "github");

        // Test getting non-existent account
        let result = repo.get_account_by_provider(&user.id, "nonexistent").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_create_and_get_verification() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        let expires_at = Utc::now() + Duration::hours(1);

        // Create verification
        let result = repo
            .create_verification(
                "email:test@example.com",
                "verification-code-123",
                expires_at,
            )
            .await;
        assert!(result.is_ok());

        let verification = result.unwrap();
        assert_eq!(verification.identifier, "email:test@example.com");
        assert_eq!(verification.value, "verification-code-123");
        assert_eq!(verification.expires_at, expires_at);

        // Get verification
        let result = repo
            .get_verification("email:test@example.com", "verification-code-123")
            .await;
        assert!(result.is_ok());

        let found_verification = result.unwrap();
        assert!(found_verification.is_some());

        let found_verification = found_verification.unwrap();
        assert_eq!(found_verification.id, verification.id);

        // Test getting non-existent verification
        let result = repo
            .get_verification("email:test@example.com", "wrong-code")
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        cleanup_test_data(&pool).await;
    }

    #[tokio::test]
    async fn test_delete_expired_verifications() {
        let pool = create_test_pool().await;
        cleanup_test_data(&pool).await;

        let repo = UserRepository::new(pool.clone());

        // Create expired verification
        let expired_expires_at = Utc::now() - Duration::hours(1);
        repo.create_verification(
            "email:expired@example.com",
            "expired-code",
            expired_expires_at,
        )
        .await
        .expect("Failed to create expired verification");

        // Create valid verification
        let valid_expires_at = Utc::now() + Duration::hours(1);
        repo.create_verification("email:valid@example.com", "valid-code", valid_expires_at)
            .await
            .expect("Failed to create valid verification");

        // Delete expired verifications
        let result = repo.delete_expired_verifications().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // Should delete 1 expired verification

        // Verify expired verification is deleted
        let result = repo
            .get_verification("email:expired@example.com", "expired-code")
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Verify valid verification still exists
        let result = repo
            .get_verification("email:valid@example.com", "valid-code")
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());

        cleanup_test_data(&pool).await;
    }
}
