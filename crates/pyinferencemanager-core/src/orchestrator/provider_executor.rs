use crate::engines::{CloudClient, OpenAIClient};
use crate::error_classifier::ErrorClassifier;
use crate::types::CloudProvider;
use crate::Result;

#[derive(Debug, Clone)]
pub struct ProviderExecutionRequest {
    pub provider: CloudProvider,
    pub prompt: String,
    pub max_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct ProviderExecutionResult {
    pub output: String,
    pub tokens_used: u32,
    pub provider_name: String,
}

pub struct ProviderExecutor;

impl ProviderExecutor {
    /// Execute request on a specific provider
    pub async fn execute(request: ProviderExecutionRequest) -> Result<ProviderExecutionResult> {
        match &request.provider {
            CloudProvider::Anthropic { model } => {
                Self::execute_anthropic(model.clone(), request.prompt, request.max_tokens).await
            }
            CloudProvider::OpenAI { model } => {
                Self::execute_openai(model.clone(), request.prompt, request.max_tokens).await
            }
        }
    }

    /// Execute on Anthropic Claude
    async fn execute_anthropic(
        model: String,
        prompt: String,
        max_tokens: u32,
    ) -> Result<ProviderExecutionResult> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| crate::Error::CloudError("ANTHROPIC_API_KEY not set".to_string()))?;

        let client = CloudClient::new(api_key, model.clone());
        let response = client.complete(&prompt, max_tokens).await?;

        Ok(ProviderExecutionResult {
            output: response.text,
            tokens_used: response.tokens_used,
            provider_name: format!("anthropic:{}", model),
        })
    }

    /// Execute on OpenAI
    async fn execute_openai(
        model: String,
        prompt: String,
        max_tokens: u32,
    ) -> Result<ProviderExecutionResult> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| crate::Error::CloudError("OPENAI_API_KEY not set".to_string()))?;

        let client = OpenAIClient::new(api_key, model.clone());
        let response = client.complete(&prompt, max_tokens).await?;

        Ok(ProviderExecutionResult {
            output: response.text,
            tokens_used: response.tokens_used,
            provider_name: format!("openai:{}", model),
        })
    }

    /// Check if error from provider execution is retryable
    pub fn is_error_retryable(error: &crate::Error) -> bool {
        match error {
            crate::Error::CloudError(msg) => {
                let status_code = ErrorClassifier::extract_status_code(msg);
                ErrorClassifier::classify(status_code, msg)
                    == crate::error_classifier::ErrorCategory::Retryable
            }
            _ => false,
        }
    }

    /// Extract provider name from error for logging
    pub fn extract_provider_from_error(_error: &crate::Error) -> Option<String> {
        // In a real implementation, would extract from error context
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_execution_request_creation() {
        let request = ProviderExecutionRequest {
            provider: CloudProvider::Anthropic {
                model: "claude-haiku-4-5".to_string(),
            },
            prompt: "Hello".to_string(),
            max_tokens: 100,
        };

        assert_eq!(request.max_tokens, 100);
    }

    #[test]
    fn test_provider_execution_result_creation() {
        let result = ProviderExecutionResult {
            output: "Response".to_string(),
            tokens_used: 50,
            provider_name: "anthropic:claude-haiku-4-5".to_string(),
        };

        assert_eq!(result.tokens_used, 50);
        assert_eq!(result.provider_name, "anthropic:claude-haiku-4-5");
    }

    #[test]
    fn test_is_error_retryable_cloud_error() {
        let error = crate::Error::CloudError("HTTP 429: Rate limit exceeded".to_string());
        assert!(ProviderExecutor::is_error_retryable(&error));

        let error = crate::Error::CloudError("HTTP 401: Unauthorized".to_string());
        assert!(!ProviderExecutor::is_error_retryable(&error));
    }

    #[test]
    fn test_is_error_retryable_non_cloud_error() {
        let error = crate::Error::CacheError("Some error".to_string());
        assert!(!ProviderExecutor::is_error_retryable(&error));
    }
}
