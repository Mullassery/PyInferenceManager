use super::{BackendKind, InferenceRequest, InferenceResult, ModelProfile, RuntimeBackend};

/// Placeholder `RuntimeBackend` for runtimes that don't have a real client
/// implementation yet. `BackendKind::VLlm` used to be one of these -- it now
/// has a real implementation in `vllm_backend::VLlmBackend` (see
/// `AnyBackend::VLlm` in `mod.rs`), so it's no longer constructed with this
/// stub. Only `TensorRtLlm` and `MlcLlm` remain genuinely unimplemented.
pub struct StubBackend {
    kind: BackendKind,
}

impl StubBackend {
    pub fn new(kind: BackendKind) -> Self {
        StubBackend { kind }
    }
}

#[async_trait::async_trait]
impl RuntimeBackend for StubBackend {
    async fn infer(&self, _request: InferenceRequest) -> crate::Result<InferenceResult> {
        Err(crate::Error::BackendError(format!(
            "{} backend not configured — no endpoint available",
            self.kind.as_str()
        )))
    }

    fn estimate_cost(&self, profile: &ModelProfile) -> f32 {
        let size_gb = profile.size_gb();
        match self.kind {
            BackendKind::TensorRtLlm => {
                if size_gb < 20.0 {
                    0.0
                } else {
                    0.00001
                }
            }
            BackendKind::MlcLlm => {
                if size_gb < 30.0 {
                    0.0
                } else {
                    0.000005
                }
            }
            _ => 0.0,
        }
    }

    fn estimate_latency(&self, profile: &ModelProfile) -> u64 {
        let size_gb = profile.size_gb();
        match self.kind {
            BackendKind::TensorRtLlm => {
                let base = (size_gb * 20.0) as u64;
                base.max(150).min(20000)
            }
            BackendKind::MlcLlm => {
                let base = (size_gb * 35.0) as u64;
                base.max(250).min(35000)
            }
            _ => 1000,
        }
    }

    fn kind(&self) -> BackendKind {
        self.kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_backend_infer_returns_error() {
        let backend = StubBackend::new(BackendKind::TensorRtLlm);
        let request = InferenceRequest {
            model: "model".to_string(),
            prompt: "test".to_string(),
            max_tokens: 100,
        };

        let result = futures::executor::block_on(backend.infer(request));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not configured"));
    }

    #[test]
    fn test_stub_backend_kind() {
        let trt = StubBackend::new(BackendKind::TensorRtLlm);
        let mlc = StubBackend::new(BackendKind::MlcLlm);

        assert_eq!(trt.kind(), BackendKind::TensorRtLlm);
        assert_eq!(mlc.kind(), BackendKind::MlcLlm);
    }

    #[test]
    fn test_stub_backend_estimate_cost() {
        let trt = StubBackend::new(BackendKind::TensorRtLlm);
        let small = ModelProfile::new(10 * 1_073_741_824, "model".to_string());
        let large = ModelProfile::new(60 * 1_073_741_824, "model".to_string());

        let small_cost = trt.estimate_cost(&small);
        let large_cost = trt.estimate_cost(&large);

        assert!(small_cost <= large_cost);
    }
}
