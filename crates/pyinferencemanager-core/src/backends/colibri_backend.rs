use super::{BackendKind, InferenceRequest, InferenceResult, ModelProfile, RuntimeBackend};

pub struct ColibriBackend;

impl ColibriBackend {
    pub fn new() -> Self {
        ColibriBackend
    }
}

#[async_trait::async_trait]
impl RuntimeBackend for ColibriBackend {
    async fn infer(&self, _request: InferenceRequest) -> crate::Result<InferenceResult> {
        Err(crate::Error::BackendError(
            "colibri backend not configured — no endpoint available".to_string(),
        ))
    }

    fn estimate_cost(&self, profile: &ModelProfile) -> f32 {
        let size_gb = profile.size_gb();
        match profile.architecture.to_lowercase().as_str() {
            s if s.contains("mixtral") || s.contains("moe") => (size_gb * 0.000001) + 0.00001,
            _ => 0.0,
        }
    }

    fn estimate_latency(&self, profile: &ModelProfile) -> u64 {
        let size_gb = profile.size_gb();
        if profile.is_moe {
            let base = (size_gb * 100.0) as u64;
            base.max(1000).min(120000)
        } else {
            let base = (size_gb * 150.0) as u64;
            base.max(2000).min(180000)
        }
    }

    fn kind(&self) -> BackendKind {
        BackendKind::Colibri
    }
}

impl Default for ColibriBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_colibri_backend_kind() {
        let backend = ColibriBackend::new();
        assert_eq!(backend.kind(), BackendKind::Colibri);
    }

    #[test]
    fn test_colibri_infer_returns_error() {
        let backend = ColibriBackend::new();
        let request = InferenceRequest {
            model: "model".to_string(),
            prompt: "test".to_string(),
            max_tokens: 100,
        };

        let result = futures::executor::block_on(backend.infer(request));
        assert!(result.is_err());
    }

    #[test]
    fn test_colibri_moe_latency_higher() {
        let backend = ColibriBackend::new();
        let non_moe = ModelProfile::new(32 * 1_073_741_824, "llama".to_string());
        let moe = ModelProfile::new(32 * 1_073_741_824, "mixtral".to_string()).with_moe(true);

        let non_moe_latency = backend.estimate_latency(&non_moe);
        let moe_latency = backend.estimate_latency(&moe);

        assert!(moe_latency < non_moe_latency);
    }

    #[test]
    fn test_colibri_estimate_cost_moe() {
        let backend = ColibriBackend::new();
        let moe_profile =
            ModelProfile::new(45 * 1_073_741_824, "mixtral".to_string()).with_moe(true);

        let cost = backend.estimate_cost(&moe_profile);
        assert!(cost > 0.0);
    }
}
