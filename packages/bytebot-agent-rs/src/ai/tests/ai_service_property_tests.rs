use super::*;
use crate::ai::{UnifiedAIService, AIService, ModelInfo};
use crate::error::AIError;
use bytebot_shared_rs::types::{message::{Message, MessageContentBlock}, task::Role};
use std::collections::HashSet;

/// Property-based tests for AI service implementations
/// These tests verify invariants and properties that should hold for all AI services
#[cfg(test)]
mod ai_service_property_tests {
    use super::*;

    /// Test that all AI services maintain consistent model information structure
    #[test]
    fn property_test_model_info_consistency() {
        let configs = vec![
            create_test_config_anthropic_only(),
            create_test_config_openai_only(),
            create_test_config_google_only(),
            create_test_config_all_providers(),
        ];

        for config in configs {
            let service = UnifiedAIService::new(&config);
            let models = service.list_models();

            for model in models {
                // Property: All model info fields must be non-empty
                assert!(!model.provider.is_empty(), "Provider must not be empty");
                assert!(!model.name.is_empty(), "Model name must not be empty");
                assert!(!model.title.is_empty(), "Model title must not be empty");

                // Property: Provider must be one of the known providers
                assert!(
                    ["anthropic", "openai", "google"].contains(&model.provider.as_str()),
                    "Provider must be one of: anthropic, openai, google"
                );

                // Property: Model name should start with provider-specific prefix
                match model.provider.as_str() {
                    "anthropic" => assert!(
                        model.name.starts_with("claude-"),
                        "Anthropic models should start with 'claude-'"
                    ),
                    "openai" => assert!(
                        model.name.starts_with("gpt-"),
                        "OpenAI models should start with 'gpt-'"
                    ),
                    "google" => assert!(
                        model.name.starts_with("gemini-"),
                        "Google models should start with 'gemini-'"
                    ),
                    _ => panic!("Unknown provider: {}", model.provider),
                }
            }
        }
    }

    /// Test that model lists are deterministic and consistent
    #[test]
    fn property_test_model_list_determinism() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        // Get models multiple times
        let models1 = service.list_models();
        let models2 = service.list_models();
        let models3 = service.list_models();

        // Property: Model lists should be identical across calls
        assert_eq!(models1.len(), models2.len());
        assert_eq!(models2.len(), models3.len());

        // Property: Model lists should contain the same models in the same order
        for (i, model) in models1.iter().enumerate() {
            assert_eq!(model.provider, models2[i].provider);
            assert_eq!(model.name, models2[i].name);
            assert_eq!(model.title, models2[i].title);

            assert_eq!(model.provider, models3[i].provider);
            assert_eq!(model.name, models3[i].name);
            assert_eq!(model.title, models3[i].title);
        }
    }

    /// Test that model names are unique within the unified service
    #[test]
    fn property_test_model_name_uniqueness() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);
        let models = service.list_models();

        let mut seen_names = HashSet::new();

        for model in models {
            // Property: Model names should be unique
            assert!(
                seen_names.insert(model.name.clone()),
                "Duplicate model name found: {}",
                model.name
            );
        }
    }

    /// Test that service availability is consistent with provider configuration
    #[test]
    fn property_test_service_availability_consistency() {
        let test_cases = vec![
            (create_test_config_all_providers(), true, 3),
            (create_test_config_anthropic_only(), true, 1),
            (create_test_config_openai_only(), true, 1),
            (create_test_config_google_only(), true, 1),
            (create_test_config_no_providers(), false, 0),
        ];

        for (config, expected_available, expected_provider_count) in test_cases {
            let service = UnifiedAIService::new(&config);

            // Property: Service availability should match provider configuration
            assert_eq!(service.is_available(), expected_available);
            assert_eq!(service.is_any_service_available(), expected_available);

            // Property: Provider count should match configuration
            let providers = service.get_available_providers();
            assert_eq!(providers.len(), expected_provider_count);

            // Property: Model count should be consistent with provider count
            let models = service.list_models();
            if expected_provider_count == 0 {
                assert_eq!(models.len(), 0);
            } else {
                assert!(models.len() > 0);
            }
        }
    }

    /// Test that model routing is consistent and predictable
    #[test]
    fn property_test_model_routing_consistency() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        let test_models = vec![
            ("claude-opus-4-20250514", "anthropic"),
            ("claude-sonnet-4-20250514", "anthropic"),
            ("gpt-4o", "openai"),
            ("gpt-4o-mini", "openai"),
            ("gpt-4-turbo", "openai"),
            ("gpt-3.5-turbo", "openai"),
            ("gemini-1.5-pro", "google"),
            ("gemini-1.5-flash", "google"),
            ("gemini-2.0-flash-exp", "google"),
        ];

        for (model_name, expected_provider) in test_models {
            // Property: Model routing should be consistent
            let result1 = service.get_service_for_model(model_name);
            let result2 = service.get_service_for_model(model_name);

            assert!(result1.is_ok(), "Model routing should succeed for {}", model_name);
            assert!(result2.is_ok(), "Model routing should succeed for {}", model_name);

            // Property: Same model should route to same service
            let service1 = result1.unwrap();
            let service2 = result2.unwrap();

            let models1 = service1.list_models();
            let models2 = service2.list_models();

            // Both should contain the same models (same service instance)
            assert_eq!(models1.len(), models2.len());

            // Property: Service should contain the requested model
            assert!(
                models1.iter().any(|m| m.name == model_name),
                "Service should contain model {}",
                model_name
            );

            // Property: All models in service should have expected provider
            for model in &models1 {
                assert_eq!(
                    model.provider, expected_provider,
                    "All models in service should have provider {}",
                    expected_provider
                );
            }
        }
    }

    /// Test that invalid model names are consistently rejected
    #[test]
    fn property_test_invalid_model_rejection() {
        let config = create_test_config_all_providers();
        let service = UnifiedAIService::new(&config);

        let invalid_models = vec![
            "",
            "invalid-model",
            "claude-invalid",
            "gpt-invalid", 
            "gemini-invalid",
            "unknown-provider-model",
            "claude",
            "gpt",
            "gemini",
            "anthropic-model",
            "openai-model",
            "google-model",
        ];

        for invalid_model in invalid_models {
            let result = service.get_service_for_model(invalid_model);
            
            // Property: Invalid models should be consistently rejected
            assert!(
                result.is_err(),
                "Invalid model '{}' should be rejected",
                invalid_model
            );

            // Property: Error should be appropriate type
            match result.err().unwrap() {
                AIError::InvalidModel(_) => {
                    // Expected for truly invalid models
                }
                AIError::Api { status, .. } => {
                    // Expected for models that route to unavailable providers
                    assert_eq!(status, 401);
                }
                _ => panic!("Unexpected error type for invalid model: {}", invalid_model),
            }
        }
    }

    /// Test that default service selection follows priority rules
    #[test]
    fn property_test_default_service_priority() {
        // Property: Anthropic should have highest priority
        let config_all = create_test_config_all_providers();
        let service_all = UnifiedAIService::new(&config_all);
        let (_, default_model_all) = service_all.get_default_service().unwrap();
        assert!(default_model_all.starts_with("claude-"));

        // Property: OpenAI should be second priority
        let config_openai_google = Config {
            anthropic_api_key: None,
            openai_api_key: Some("test-key".to_string()),
            google_api_key: Some("test-key".to_string()),
            ..Default::default()
        };
        let service_openai_google = UnifiedAIService::new(&config_openai_google);
        let (_, default_model_openai_google) = service_openai_google.get_default_service().unwrap();
        assert!(default_model_openai_google.starts_with("gpt-"));

        // Property: Google should be lowest priority
        let config_google_only = create_test_config_google_only();
        let service_google_only = UnifiedAIService::new(&config_google_only);
        let (_, default_model_google) = service_google_only.get_default_service().unwrap();
        assert!(default_model_google.starts_with("gemini-"));
    }

    /// Test that message content validation is consistent
    #[test]
    fn property_test_message_content_validation() {
        let test_contents = vec![
            create_test_text_content("Simple text"),
            create_test_image_content("image/png", "base64data"),
            create_test_tool_use_content("calculator", "calc_1", serde_json::json!({"op": "add"})),
            create_test_tool_result_content("calc_1", "Result: 3", false),
            create_test_mixed_content(),
        ];

        for content in test_contents {
            // Property: All content types should be valid for message creation
            let message = Message::new(content.clone(), Role::User, "test-task-id".to_string());
            
            // Property: Message should preserve content structure
            let retrieved_content = message.get_content_blocks().unwrap();
            assert_eq!(retrieved_content.len(), content.len());

            // Property: Content blocks should maintain their types
            for (original, retrieved) in content.iter().zip(retrieved_content.iter()) {
                match (original, retrieved) {
                    (MessageContentBlock::Text { text: t1 }, MessageContentBlock::Text { text: t2 }) => {
                        assert_eq!(t1, t2);
                    }
                    (MessageContentBlock::Image { source: s1 }, MessageContentBlock::Image { source: s2 }) => {
                        assert_eq!(s1.media_type, s2.media_type);
                        assert_eq!(s1.data, s2.data);
                    }
                    (MessageContentBlock::ToolUse { name: n1, id: i1, input: inp1 }, 
                     MessageContentBlock::ToolUse { name: n2, id: i2, input: inp2 }) => {
                        assert_eq!(n1, n2);
                        assert_eq!(i1, i2);
                        assert_eq!(inp1, inp2);
                    }
                    _ => {
                        // For other types, just verify they're the same variant
                        assert_eq!(
                            std::mem::discriminant(original),
                            std::mem::discriminant(retrieved)
                        );
                    }
                }
            }
        }
    }

    /// Test that error handling is consistent across different scenarios
    #[test]
    fn property_test_error_handling_consistency() {
        // Test with no providers
        let config_none = create_test_config_no_providers();
        let service_none = UnifiedAIService::new(&config_none);

        // Property: No providers should result in consistent error
        let default_result = service_none.get_default_service();
        assert!(default_result.is_err());
        match default_result.err().unwrap() {
            AIError::Api { status, .. } => assert_eq!(status, 503),
            _ => panic!("Expected API error for no providers"),
        }

        // Test with partial providers
        let config_partial = create_test_config_anthropic_only();
        let service_partial = UnifiedAIService::new(&config_partial);

        // Property: Unavailable providers should result in consistent error
        let unavailable_result = service_partial.get_service_for_model("gpt-4o");
        assert!(unavailable_result.is_err());
        match unavailable_result.err().unwrap() {
            AIError::Api { status, .. } => assert_eq!(status, 401),
            _ => panic!("Expected API error for unavailable provider"),
        }

        // Property: Available providers should work
        let available_result = service_partial.get_service_for_model("claude-opus-4-20250514");
        assert!(available_result.is_ok());
    }

    /// Test serialization and deserialization properties
    #[test]
    fn property_test_serialization_roundtrip() {
        let test_contents = vec![
            create_test_text_content("Test text with special chars: àáâãäå"),
            create_test_image_content("image/jpeg", "SGVsbG8gV29ybGQ="),
            create_test_tool_use_content(
                "complex_tool",
                "tool_123",
                serde_json::json!({
                    "nested": {
                        "array": [1, 2, 3],
                        "string": "value",
                        "boolean": true,
                        "null": null
                    }
                })
            ),
        ];

        for original_content in test_contents {
            // Property: Content should survive JSON serialization roundtrip
            let json_value = serde_json::to_value(&original_content).unwrap();
            let deserialized_content: Vec<MessageContentBlock> = 
                serde_json::from_value(json_value).unwrap();

            assert_eq!(original_content.len(), deserialized_content.len());

            for (original, deserialized) in original_content.iter().zip(deserialized_content.iter()) {
                // Property: Serialized content should be identical to original
                let original_json = serde_json::to_value(original).unwrap();
                let deserialized_json = serde_json::to_value(deserialized).unwrap();
                assert_eq!(original_json, deserialized_json);
            }
        }
    }

    /// Test that model information is stable across service instances
    #[test]
    fn property_test_model_info_stability() {
        let config = create_test_config_all_providers();
        
        // Create multiple service instances
        let service1 = UnifiedAIService::new(&config);
        let service2 = UnifiedAIService::new(&config);
        let service3 = UnifiedAIService::new(&config);

        let models1 = service1.list_models();
        let models2 = service2.list_models();
        let models3 = service3.list_models();

        // Property: Model information should be stable across instances
        assert_eq!(models1.len(), models2.len());
        assert_eq!(models2.len(), models3.len());

        for i in 0..models1.len() {
            assert_eq!(models1[i].provider, models2[i].provider);
            assert_eq!(models1[i].name, models2[i].name);
            assert_eq!(models1[i].title, models2[i].title);

            assert_eq!(models1[i].provider, models3[i].provider);
            assert_eq!(models1[i].name, models3[i].name);
            assert_eq!(models1[i].title, models3[i].title);
        }
    }
}