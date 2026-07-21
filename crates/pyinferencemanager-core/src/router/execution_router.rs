use crate::types::{CloudProvider, ExecutionEngine, ExecutionMode, HardwareProfile, PrivacyLevel};

#[derive(Debug, Clone)]
pub struct ExecutionRouter {
    pub complexity_cloud_threshold: f32,
    pub complexity_local_threshold: f32,
    pub execution_mode: ExecutionMode,
}

impl ExecutionRouter {
    pub fn new(execution_mode: ExecutionMode) -> Self {
        ExecutionRouter {
            complexity_cloud_threshold: 0.7,
            complexity_local_threshold: 0.3,
            execution_mode,
        }
    }

    pub fn with_thresholds(mut self, cloud: f32, local: f32) -> Self {
        self.complexity_cloud_threshold = cloud;
        self.complexity_local_threshold = local;
        self
    }

    pub fn select_engine(
        &self,
        complexity: f32,
        privacy: &PrivacyLevel,
        cache_hit: bool,
        hardware: &HardwareProfile,
    ) -> ExecutionEngine {
        if cache_hit {
            return ExecutionEngine::CacheLookup;
        }

        if *privacy == PrivacyLevel::High {
            return ExecutionEngine::LocalLlm {
                model: hardware
                    .best_available_model
                    .clone()
                    .unwrap_or_else(|| "llama3.2:latest".to_string()),
            };
        }

        match self.execution_mode {
            ExecutionMode::LocalFirst => self.route_local_first(complexity, hardware),
            ExecutionMode::CloudFirst => self.route_cloud_first(complexity, hardware),
        }
    }

    fn route_local_first(
        &self,
        complexity: f32,
        hardware: &HardwareProfile,
    ) -> ExecutionEngine {
        if complexity > self.complexity_cloud_threshold {
            return ExecutionEngine::CloudLlm {
                provider: CloudProvider::Anthropic {
                    model: "claude-haiku-4-5".to_string(),
                },
            };
        }

        if let Some(model) = &hardware.best_available_model {
            ExecutionEngine::LocalLlm {
                model: model.clone(),
            }
        } else {
            ExecutionEngine::CloudLlm {
                provider: CloudProvider::Anthropic {
                    model: "claude-haiku-4-5".to_string(),
                },
            }
        }
    }

    fn route_cloud_first(
        &self,
        complexity: f32,
        hardware: &HardwareProfile,
    ) -> ExecutionEngine {
        if complexity < self.complexity_local_threshold {
            if let Some(model) = &hardware.best_available_model {
                return ExecutionEngine::LocalLlm {
                    model: model.clone(),
                };
            }
        }

        self.select_best_cloud_provider(complexity)
    }

    fn select_best_cloud_provider(&self, complexity: f32) -> ExecutionEngine {
        if complexity > 0.8 {
            ExecutionEngine::CloudLlm {
                provider: CloudProvider::Anthropic {
                    model: "claude-opus-4-1".to_string(),
                },
            }
        } else if complexity > 0.5 {
            ExecutionEngine::CloudLlm {
                provider: CloudProvider::Anthropic {
                    model: "claude-haiku-4-5".to_string(),
                },
            }
        } else {
            ExecutionEngine::CloudLlm {
                provider: CloudProvider::OpenAI {
                    model: "gpt-4o-mini".to_string(),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_hardware() -> HardwareProfile {
        let mut profile = HardwareProfile::new(16 * 1_073_741_824);
        profile.best_available_model = Some("llama3.2:latest".to_string());
        profile
    }

    #[test]
    fn test_execution_router_new() {
        let router = ExecutionRouter::new(ExecutionMode::LocalFirst);
        assert_eq!(router.execution_mode, ExecutionMode::LocalFirst);
        assert_eq!(router.complexity_cloud_threshold, 0.7);
    }

    #[test]
    fn test_router_with_thresholds() {
        let router = ExecutionRouter::new(ExecutionMode::LocalFirst)
            .with_thresholds(0.6, 0.2);

        assert_eq!(router.complexity_cloud_threshold, 0.6);
        assert_eq!(router.complexity_local_threshold, 0.2);
    }

    #[test]
    fn test_cache_hit_returns_cache_lookup() {
        let router = ExecutionRouter::new(ExecutionMode::LocalFirst);
        let hardware = create_hardware();

        let engine = router.select_engine(0.8, &PrivacyLevel::Low, true, &hardware);
        assert!(matches!(engine, ExecutionEngine::CacheLookup));
    }

    #[test]
    fn test_privacy_high_forces_local() {
        let router = ExecutionRouter::new(ExecutionMode::CloudFirst);
        let hardware = create_hardware();

        let engine = router.select_engine(0.9, &PrivacyLevel::High, false, &hardware);
        match engine {
            ExecutionEngine::LocalLlm { model } => {
                assert_eq!(model, "llama3.2:latest");
            }
            _ => panic!("Expected LocalLlm"),
        }
    }

    #[test]
    fn test_local_first_low_complexity() {
        let router = ExecutionRouter::new(ExecutionMode::LocalFirst);
        let hardware = create_hardware();

        let engine = router.select_engine(0.2, &PrivacyLevel::Low, false, &hardware);
        match engine {
            ExecutionEngine::LocalLlm { .. } => {}
            _ => panic!("Expected LocalLlm for low complexity"),
        }
    }

    #[test]
    fn test_local_first_high_complexity() {
        let router = ExecutionRouter::new(ExecutionMode::LocalFirst);
        let hardware = create_hardware();

        let engine = router.select_engine(0.8, &PrivacyLevel::Low, false, &hardware);
        match engine {
            ExecutionEngine::CloudLlm { provider } => {
                match provider {
                    CloudProvider::Anthropic { model } => {
                        assert_eq!(model, "claude-haiku-4-5");
                    }
                    _ => panic!("Expected Anthropic provider"),
                }
            }
            _ => panic!("Expected CloudLlm for high complexity"),
        }
    }

    #[test]
    fn test_local_first_no_local_model() {
        let mut router = ExecutionRouter::new(ExecutionMode::LocalFirst);
        router.complexity_cloud_threshold = 0.7;

        let mut hardware = create_hardware();
        hardware.best_available_model = None;

        let engine = router.select_engine(0.5, &PrivacyLevel::Low, false, &hardware);
        match engine {
            ExecutionEngine::CloudLlm { .. } => {}
            _ => panic!("Expected CloudLlm fallback when no local model"),
        }
    }

    #[test]
    fn test_cloud_first_high_complexity() {
        let router = ExecutionRouter::new(ExecutionMode::CloudFirst);
        let hardware = create_hardware();

        let engine = router.select_engine(0.9, &PrivacyLevel::Low, false, &hardware);
        match engine {
            ExecutionEngine::CloudLlm { .. } => {}
            _ => panic!("Expected CloudLlm for high complexity in CloudFirst"),
        }
    }

    #[test]
    fn test_cloud_first_low_complexity() {
        let router = ExecutionRouter::new(ExecutionMode::CloudFirst);
        let hardware = create_hardware();

        let engine = router.select_engine(0.2, &PrivacyLevel::Low, false, &hardware);
        match engine {
            ExecutionEngine::LocalLlm { .. } => {}
            _ => panic!("Expected LocalLlm for low complexity in CloudFirst"),
        }
    }

    #[test]
    fn test_cloud_first_mid_complexity() {
        let router = ExecutionRouter::new(ExecutionMode::CloudFirst);
        let hardware = create_hardware();

        let engine = router.select_engine(0.5, &PrivacyLevel::Low, false, &hardware);
        match engine {
            ExecutionEngine::CloudLlm { .. } => {}
            _ => panic!("Expected CloudLlm for mid complexity in CloudFirst"),
        }
    }

    #[test]
    fn test_select_best_cloud_provider_high_complexity() {
        let router = ExecutionRouter::new(ExecutionMode::CloudFirst);
        let engine = router.select_best_cloud_provider(0.85);
        match engine {
            ExecutionEngine::CloudLlm {
                provider: CloudProvider::Anthropic { model },
            } => {
                assert_eq!(model, "claude-opus-4-1");
            }
            _ => panic!("Expected Anthropic Opus for high complexity"),
        }
    }

    #[test]
    fn test_select_best_cloud_provider_mid_complexity() {
        let router = ExecutionRouter::new(ExecutionMode::CloudFirst);
        let engine = router.select_best_cloud_provider(0.6);
        match engine {
            ExecutionEngine::CloudLlm {
                provider: CloudProvider::Anthropic { model },
            } => {
                assert_eq!(model, "claude-haiku-4-5");
            }
            _ => panic!("Expected Anthropic Haiku for mid complexity"),
        }
    }

    #[test]
    fn test_select_best_cloud_provider_low_complexity() {
        let router = ExecutionRouter::new(ExecutionMode::CloudFirst);
        let engine = router.select_best_cloud_provider(0.2);
        match engine {
            ExecutionEngine::CloudLlm {
                provider: CloudProvider::OpenAI { model },
            } => {
                assert_eq!(model, "gpt-4o-mini");
            }
            _ => panic!("Expected OpenAI for low complexity"),
        }
    }
}
