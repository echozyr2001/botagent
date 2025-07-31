use super::*;
use crate::ai::{UnifiedAIService, AIService, ModelInfo};
use crate::error::AIError;
use bytebot_shared_rs::types::{message::Message, task::Role};

/// Test UnifiedAIService creation and basic functionality
#[cfg(test)]
mod unified_ai_service_tests {
    use super::*;

    #[test]
    fn test_unified_ai_service_creation_with_all_providers() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        assert!(service.is_available());
        assert!(service.is_any_service_available());

        let providers = service.get_available_providers();
        assert_eq!(providers.len(), 3);
        assert!(providers.contains(&"anthropic".to_string()));
        assert!(providers.contains(&"openai".to_string()));
        assert!(providers.contains(&"google".to_string()));
    }

    #[test]
    fn test_unified_ai_service_creation_with_no_providers() {
        let config = create_test_config_no_providers();
        let service = UnifiedAIService::new(&config);

        assert!(!service.is_available());
        assert!(!service.is_any_service_available());

        let providers = service.get_available_providers();
        assert_eq!(providers.len(), 0);
    }

    #[test]
    fn test_unified_ai_service_creation_with_single_provider() {
        let config = create_test_config_anthropic_only();
        let service = UnifiedAIService::new(&config);

        assert!(service.is_available());

        let providers = service.get_available_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0], "anthropic");
    }

    #[test]
    fn test_list_all_models_with_all_providers() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        let models = service.list_all_models();

        // Should have models from all three providers
        // Anthropic: 2 models, OpenAI: 4 models, Google: 3 models = 9 total
        assert_eq!(models.len(), 9);

        // Check that we have models from each provider
        let anthropic_models: Vec<_> = models
            .iter()
            .filter(|m| m.provider == "anthropic")
            .collect();
        let openai_models: Vec<_> = models.iter().filter(|m| m.provider == "openai").collect();
        let google_models: Vec<_> = models.iter().filter(|m| m.provider == "google").collect();

        assert_eq!(anthropic_models.len(), 2);
        assert_eq!(openai_models.len(), 4);
        assert_eq!(google_models.len(), 3);
    }

    #[test]
    fn test_list_all_models_with_single_provider() {
        let config = create_test_config_openai_only();
        let service = UnifiedAIService::new(&config);

        let models = service.list_all_models();

        // Should only have OpenAI models
        assert_eq!(models.len(), 4);
        assert!(models.iter().all(|m| m.provider == "openai"));
    }

    #[test]
    fn test_list_all_models_with_no_providers() {
        let config = create_test_config_no_providers();
        let service = UnifiedAIService::new(&config);

        let models = service.list_all_models();
        assert_eq!(models.len(), 0);
    }

    #[test]
    fn test_get_service_for_model_anthropic() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        let result = service.get_service_for_model("claude-opus-4-20250514");
        assert!(result.is_ok());

        let ai_service = result.unwrap();
        let models = ai_service.list_models();
        assert!(models.iter().any(|m| m.name == "claude-opus-4-20250514"));
    }

    #[test]
    fn test_get_service_for_model_openai() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        let result = service.get_service_for_model("gpt-4o");
        assert!(result.is_ok());

        let ai_service = result.unwrap();
        let models = ai_service.list_models();
        assert!(models.iter().any(|m| m.name == "gpt-4o"));
    }

    #[test]
    fn test_get_service_for_model_google() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        let result = service.get_service_for_model("gemini-1.5-pro");
        assert!(result.is_ok());

        let ai_service = result.unwrap();
        let models = ai_service.list_models();
        assert!(models.iter().any(|m| m.name == "gemini-1.5-pro"));
    }

    #[test]
    fn test_get_service_for_model_invalid() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        let result = service.get_service_for_model("invalid-model");
        assert!(result.is_err());

        match result.err().unwrap() {
            AIError::InvalidModel(msg) => {
                assert!(msg.contains("Unknown model: invalid-model"));
            }
            _ => panic!("Expected InvalidModel error"),
        }
    }

    #[test]
    fn test_get_service_for_model_unavailable_provider() {
        let config = create_test_config_openai_only(); // Only OpenAI available
        let service = UnifiedAIService::new(&config);

        let result = service.get_service_for_model("claude-opus-4-20250514");
        assert!(result.is_err());

        match result.err().unwrap() {
            AIError::Api { status, message } => {
                assert_eq!(status, 401);
                assert!(message.contains("Anthropic API key not configured"));
            }
            _ => panic!("Expected API error"),
        }
    }

    #[test]
    fn test_get_default_service_anthropic_priority() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        let result = service.get_default_service();
        assert!(result.is_ok());

        let (_, default_model) = result.unwrap();
        assert_eq!(default_model, "claude-opus-4-20250514"); // Anthropic has priority
    }

    #[test]
    fn test_get_default_service_openai_fallback() {
        let config = create_test_config_openai_only();
        let service = UnifiedAIService::new(&config);

        let result = service.get_default_service();
        assert!(result.is_ok());

        let (_, default_model) = result.unwrap();
        assert_eq!(default_model, "gpt-4o"); // OpenAI fallback
    }

    #[test]
    fn test_get_default_service_google_fallback() {
        let config = create_test_config_google_only();
        let service = UnifiedAIService::new(&config);

        let result = service.get_default_service();
        assert!(result.is_ok());

        let (_, default_model) = result.unwrap();
        assert_eq!(default_model, "gemini-1.5-pro"); // Google fallback
    }

    #[test]
    fn test_get_default_service_no_providers() {
        let config = create_test_config_no_providers();
        let service = UnifiedAIService::new(&config);

        let result = service.get_default_service();
        assert!(result.is_err());

        match result.err().unwrap() {
            AIError::Api { status, message } => {
                assert_eq!(status, 503);
                assert!(message.contains("No AI services are available"));
            }
            _ => panic!("Expected API error"),
        }
    }

    #[test]
    fn test_ai_service_trait_implementation() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        // Test trait methods
        assert!(service.is_available());

        let models = service.list_models();
        assert_eq!(models.len(), 9); // All models from all providers

        // Verify model information structure
        for model in &models {
            assert!(!model.provider.is_empty());
            assert!(!model.name.is_empty());
            assert!(!model.title.is_empty());
        }
    }

    /// Integration tests that demonstrate the unified AI service functionality
    /// These tests don't make actual API calls but verify the service routing
    #[tokio::test]
    async fn test_unified_service_model_routing() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        // Test that different models route to correct services
        let test_cases = vec![
            ("claude-opus-4-20250514", "anthropic"),
            ("claude-sonnet-4-20250514", "anthropic"),
            ("gpt-4o", "openai"),
            ("gpt-4o-mini", "openai"),
            ("gemini-1.5-pro", "google"),
            ("gemini-1.5-flash", "google"),
        ];

        for (model_name, expected_provider) in test_cases {
            let service_result = service.get_service_for_model(model_name);
            assert!(
                service_result.is_ok(),
                "Failed to get service for model: {model_name}"
            );

            let ai_service = service_result.unwrap();
            let models = ai_service.list_models();

            // Verify the service has the expected model and provider
            let model_found = models.iter().find(|m| m.name == model_name);
            assert!(
                model_found.is_some(),
                "Model {model_name} not found in service"
            );

            let model_info = model_found.unwrap();
            assert_eq!(model_info.provider, expected_provider);
        }
    }

    #[tokio::test]
    async fn test_unified_service_provider_availability() {
        // Test with different provider configurations
        let test_configs = vec![
            (create_test_config_anthropic_only(), vec!["anthropic"]),
            (create_test_config_openai_only(), vec!["openai"]),
            (create_test_config_google_only(), vec!["google"]),
            (
                create_test_config_all_providers(),
                vec!["anthropic", "openai", "google"],
            ),
            (create_test_config_no_providers(), vec![]),
        ];

        for (config, expected_providers) in test_configs {
            let service = UnifiedAIService::new(&config);
            let available_providers = service.get_available_providers();

            assert_eq!(available_providers.len(), expected_providers.len());
            for expected_provider in expected_providers {
                assert!(available_providers.contains(&expected_provider.to_string()));
            }

            assert_eq!(service.is_available(), !available_providers.is_empty());
        }
    }

    #[tokio::test]
    async fn test_unified_service_model_aggregation() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        let all_models = service.list_all_models();

        // Verify we have the expected models from each provider
        let expected_models = vec![
            // Anthropic models
            ("anthropic", "claude-opus-4-20250514", "Claude Opus 4"),
            ("anthropic", "claude-sonnet-4-20250514", "Claude Sonnet 4"),
            // OpenAI models
            ("openai", "gpt-4o", "GPT-4o"),
            ("openai", "gpt-4o-mini", "GPT-4o Mini"),
            ("openai", "gpt-4-turbo", "GPT-4 Turbo"),
            ("openai", "gpt-3.5-turbo", "GPT-3.5 Turbo"),
            // Google models
            ("google", "gemini-1.5-pro", "Gemini 1.5 Pro"),
            ("google", "gemini-1.5-flash", "Gemini 1.5 Flash"),
            (
                "google",
                "gemini-2.0-flash-exp",
                "Gemini 2.0 Flash (Experimental)",
            ),
        ];

        assert_eq!(all_models.len(), expected_models.len());

        for (expected_provider, expected_name, expected_title) in expected_models {
            let model_found = all_models
                .iter()
                .find(|m| m.provider == expected_provider && m.name == expected_name);

            assert!(model_found.is_some(),
                "Expected model not found: {expected_provider} {expected_name} {expected_title}");

            let model = model_found.unwrap();
            assert_eq!(model.title, expected_title);
        }
    }

    #[tokio::test]
    async fn test_unified_service_error_handling() {
        let config = create_test_config_no_providers();
        let service = UnifiedAIService::new(&config);

        // Test that service correctly reports unavailability
        assert!(!service.is_available());
        assert_eq!(service.list_models().len(), 0);

        // Test that generate_response fails appropriately
        let messages = vec![Message::new(
            create_test_text_content("Test message"),
            Role::User,
            "test-task-id".to_string(),
        )];

        let result = service
            .generate_response(
                "Test prompt",
                messages,
                None, // No model specified, should try default
                false,
                None,
            )
            .await;

        assert!(result.is_err());
        match result.err().unwrap() {
            AIError::Api { status, message } => {
                assert_eq!(status, 503);
                assert!(message.contains("No AI services are available"));
            }
            _ => panic!("Expected API error for unavailable services"),
        }
    }

    #[tokio::test]
    async fn test_unified_service_partial_availability() {
        // Test with only OpenAI available
        let config = create_test_config_openai_only();
        let service = UnifiedAIService::new(&config);

        assert!(service.is_available());

        // Should be able to use OpenAI models
        let openai_service = service.get_service_for_model("gpt-4o");
        assert!(openai_service.is_ok());

        // Should fail for Anthropic models
        let anthropic_result = service.get_service_for_model("claude-opus-4-20250514");
        assert!(anthropic_result.is_err());

        // Should fail for Google models
        let google_result = service.get_service_for_model("gemini-1.5-pro");
        assert!(google_result.is_err());

        // Default should use OpenAI
        let (_, default_model) = service.get_default_service().unwrap();
        assert_eq!(default_model, "gpt-4o");
    }

    #[tokio::test]
    async fn test_unified_service_model_validation() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        // Test valid models
        let valid_models = vec!["claude-opus-4-20250514", "gpt-4o", "gemini-1.5-pro"];

        for model in valid_models {
            let result = service.get_service_for_model(model);
            assert!(result.is_ok(), "Valid model {model} should be accepted");
        }

        // Test invalid models
        let invalid_models = vec!["invalid-model", ""];

        for model in invalid_models {
            let result = service.get_service_for_model(model);
            assert!(result.is_err(), "Invalid model {model} should be rejected");

            match result.err().unwrap() {
                AIError::InvalidModel(_) => {} // Expected
                _ => panic!("Expected InvalidModel error for: {model}"),
            }
        }
    }
}