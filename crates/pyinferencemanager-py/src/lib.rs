use pyinferencemanager_core::backends::BackendKind;
use pyinferencemanager_core::optimizer::{BackoffStrategy, BudgetConfig, BudgetStatus, RetryConfig};
use pyinferencemanager_core::orchestrator::{RealLoadTestConfig, RealLoadTester};
use pyinferencemanager_core::{ExecutionMode, Orchestrator, OrchestratorConfig};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::sync::Arc;
use std::sync::Mutex;

// ============================================================================
// BACKEND ENUM
// ============================================================================

#[pyclass]
pub struct PyBackendKind {
    inner: BackendKind,
}

#[pymethods]
impl PyBackendKind {
    #[staticmethod]
    fn anthropic() -> Self {
        PyBackendKind { inner: BackendKind::Anthropic }
    }

    #[staticmethod]
    fn openai() -> Self {
        PyBackendKind { inner: BackendKind::OpenAi }
    }

    #[staticmethod]
    fn gemini() -> Self {
        PyBackendKind { inner: BackendKind::Gemini }
    }

    #[staticmethod]
    fn ollama() -> Self {
        PyBackendKind { inner: BackendKind::Ollama }
    }

    #[staticmethod]
    fn vllm() -> Self {
        PyBackendKind { inner: BackendKind::VLlm }
    }

    #[staticmethod]
    fn tensorrt_llm() -> Self {
        PyBackendKind { inner: BackendKind::TensorRtLlm }
    }

    #[staticmethod]
    fn mlc_llm() -> Self {
        PyBackendKind { inner: BackendKind::MlcLlm }
    }

    #[staticmethod]
    fn colibri() -> Self {
        PyBackendKind { inner: BackendKind::Colibri }
    }

    fn as_str(&self) -> String {
        self.inner.as_str().to_string()
    }
}

// ============================================================================
// HARDWARE PROFILE CLASS
// ============================================================================

#[pyclass]
pub struct PyHardwareProfile {
    total_memory_bytes: u64,
    memory_tier: String,
    recommended_model_tier: String,
    is_apple_silicon: bool,
    has_metal: bool,
    available_ollama_models: Vec<String>,
    best_available_model: Option<String>,
}

#[pymethods]
impl PyHardwareProfile {
    #[getter]
    fn total_memory_gb(&self) -> u64 {
        self.total_memory_bytes / 1_073_741_824
    }

    #[getter]
    fn memory_tier(&self) -> String {
        self.memory_tier.clone()
    }

    #[getter]
    fn recommended_model_tier(&self) -> String {
        self.recommended_model_tier.clone()
    }

    #[getter]
    fn is_apple_silicon(&self) -> bool {
        self.is_apple_silicon
    }

    #[getter]
    fn has_metal(&self) -> bool {
        self.has_metal
    }

    #[getter]
    fn available_ollama_models(&self) -> Vec<String> {
        self.available_ollama_models.clone()
    }

    #[getter]
    fn best_available_model(&self) -> Option<String> {
        self.best_available_model.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "HardwareProfile(memory={}GB, tier={}, model_tier={})",
            self.total_memory_gb(),
            self.memory_tier,
            self.recommended_model_tier
        )
    }
}

// ============================================================================
// ORCHESTRATOR CLASS (EXPANDED)
// ============================================================================

#[pyclass]
pub struct PyOrchestrator {
    inner: Arc<Mutex<Orchestrator>>,
    runtime: tokio::runtime::Runtime,
}

#[pymethods]
impl PyOrchestrator {
    #[new]
    #[pyo3(signature = (mode = "local_first"))]
    fn new(py: Python<'_>, mode: &str) -> PyResult<Self> {
        let execution_mode = match mode {
            "local_first" => ExecutionMode::LocalFirst,
            "cloud_first" => ExecutionMode::CloudFirst,
            _ => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "mode must be 'local_first' or 'cloud_first'",
                ))
            }
        };

        let config = OrchestratorConfig::default().with_execution_mode(execution_mode);

        // Construction does real filesystem I/O (SQLite cache setup) — release
        // the GIL so other Python threads aren't blocked while that happens.
        py.allow_threads(|| {
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

            let orchestrator = runtime
                .block_on(Orchestrator::new(config))
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

            Ok(PyOrchestrator {
                inner: Arc::new(Mutex::new(orchestrator)),
                runtime,
            })
        })
    }

    #[pyo3(signature = (task, file=None, message=None, privacy="low"))]
    pub fn run(
        &self,
        py: Python<'_>,
        task: &str,
        file: Option<&str>,
        message: Option<&str>,
        privacy: &str,
    ) -> PyResult<PyWorkloadResult> {
        use pyinferencemanager_core::types::{Attachment, AttachmentKind, PrivacyLevel, Task};

        let privacy_level = match privacy {
            "high" => PrivacyLevel::High,
            "low" => PrivacyLevel::Low,
            _ => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "privacy must be 'high' or 'low'",
                ))
            }
        };

        // Copy everything out of the borrowed Python string args *before*
        // releasing the GIL below — nothing inside allow_threads may touch
        // Python-owned memory.
        let task_owned = task.to_string();
        let file_owned = file.map(|s| s.to_string());
        let message_owned = message.map(|s| s.to_string());

        let mut py_task = Task::new(task_owned).with_options(
            pyinferencemanager_core::types::TaskOptions {
                privacy: privacy_level,
                ..Default::default()
            },
        );

        if let Some(file_path) = &file_owned {
            if let Ok(content) = std::fs::read(file_path) {
                let attachment = Attachment {
                    kind: AttachmentKind::File,
                    content,
                    mime_type: "application/octet-stream".to_string(),
                    name: file_path.clone(),
                };
                py_task = py_task.with_attachment(attachment);
            }
        }

        if let Some(msg) = &message_owned {
            let attachment = Attachment {
                kind: AttachmentKind::RawText,
                content: msg.as_bytes().to_vec(),
                mime_type: "text/plain".to_string(),
                name: "message".to_string(),
            };
            py_task = py_task.with_attachment(attachment);
        }

        let inner = self.inner.clone();

        // The real work here is a live HTTP call to Ollama and/or a cloud
        // provider (possibly several, with retry backoff sleeps) — this can
        // take seconds. Releasing the GIL lets other Python threads run
        // (e.g. concurrent orchestrator calls, or just an unrelated UI
        // thread) instead of being serialized behind this one blocking call.
        py.allow_threads(move || {
            let orchestrator = inner.lock().map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                    "Failed to acquire orchestrator lock",
                )
            })?;

            let result = self
                .runtime
                .block_on(orchestrator.execute(py_task))
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

            Ok(PyWorkloadResult { inner: result })
        })
    }

    pub fn plan(&self, py: Python<'_>, task: &str) -> PyResult<PyExecutionPlan> {
        use pyinferencemanager_core::types::Task;

        let task_owned = task.to_string();

        py.allow_threads(move || {
            let py_task = Task::new(task_owned);

            let orchestrator = self.inner.lock().map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                    "Failed to acquire orchestrator lock",
                )
            })?;

            let plan = self
                .runtime
                .block_on(orchestrator.plan(&py_task))
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

            Ok(PyExecutionPlan {
                stages: plan.stages.len() as u32,
                estimated_cost_usd: plan.estimated_cost_usd,
                estimated_latency_ms: plan.estimated_latency_ms,
                local_first: plan.local_first,
            })
        })
    }

    pub fn provider_ranking(&self) -> PyResult<Vec<(String, f32)>> {
        let orchestrator = self.inner.lock().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Failed to acquire orchestrator lock")
        })?;

        Ok(orchestrator.provider_ranking())
    }

    /// Real-time per-provider performance metrics (success rate, average
    /// latency, cost/1k tokens, health score, request count) as tracked by
    /// the dynamic router from actual completed calls.
    pub fn provider_performance(&self, py: Python<'_>) -> PyResult<PyObject> {
        let orchestrator = self.inner.lock().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Failed to acquire orchestrator lock")
        })?;

        let perf = orchestrator.provider_performance();
        let dict = PyDict::new_bound(py);
        for (name, metrics) in perf {
            let entry = PyDict::new_bound(py);
            entry.set_item("success_rate", metrics.success_rate)?;
            entry.set_item("avg_latency_ms", metrics.avg_latency_ms)?;
            entry.set_item("cost_per_1k_tokens", metrics.cost_per_1k_tokens)?;
            entry.set_item("health_score", metrics.health_score)?;
            entry.set_item("request_count", metrics.request_count)?;
            entry.set_item("total_cost_usd", metrics.total_cost_usd)?;
            dict.set_item(name, entry)?;
        }
        Ok(dict.into())
    }

    /// Configure cost guardrails: a hard/soft spend cap enforced on every
    /// real cloud-provider call made via `run()`. When `enforce_hard_limit`
    /// is true and the cap is reached, further cloud calls are refused
    /// (raising) rather than silently spending past the limit.
    #[pyo3(signature = (max_cost_usd=100.0, max_requests=1000, alert_threshold_percent=80.0, enforce_hard_limit=true))]
    pub fn configure_budget(
        &self,
        max_cost_usd: f32,
        max_requests: u32,
        alert_threshold_percent: f32,
        enforce_hard_limit: bool,
    ) -> PyResult<()> {
        let mut orchestrator = self.inner.lock().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Failed to acquire orchestrator lock")
        })?;

        orchestrator.set_budget_config(BudgetConfig {
            max_cost_usd,
            max_requests,
            alert_threshold_percent,
            enforce_hard_limit,
        });
        Ok(())
    }

    /// Current spend/alerts/remaining budget against the configured cap.
    pub fn budget_status(&self, py: Python<'_>) -> PyResult<PyObject> {
        let orchestrator = self.inner.lock().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Failed to acquire orchestrator lock")
        })?;

        let status: BudgetStatus = orchestrator.budget_status();
        let dict = PyDict::new_bound(py);
        dict.set_item("current_cost_usd", status.current_cost_usd)?;
        dict.set_item("max_cost_usd", status.max_cost_usd)?;
        dict.set_item("percent_used", status.percent_used)?;
        dict.set_item("remaining_budget_usd", status.remaining_budget_usd)?;
        dict.set_item("current_requests", status.current_requests)?;
        dict.set_item("max_requests", status.max_requests)?;
        dict.set_item("within_budget", status.within_budget)?;
        let alerts: Vec<String> = status.alerts.iter().map(|a| a.message.clone()).collect();
        dict.set_item("alerts", alerts)?;
        Ok(dict.into())
    }

    /// Configure the retry/backoff policy used when a real cloud-provider
    /// call fails with a retryable error (HTTP 429/408/5xx).
    /// `backoff` is one of "fixed", "linear", or "exponential".
    #[pyo3(signature = (max_attempts=3, backoff="exponential", initial_ms=100, max_ms=5000))]
    pub fn configure_retry(
        &self,
        max_attempts: u32,
        backoff: &str,
        initial_ms: u64,
        max_ms: u64,
    ) -> PyResult<()> {
        let strategy = match backoff {
            "fixed" => BackoffStrategy::Fixed { delay_ms: initial_ms },
            "linear" => BackoffStrategy::Linear { increment_ms: initial_ms, max_ms },
            "exponential" => BackoffStrategy::Exponential { initial_ms, max_ms },
            _ => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "backoff must be 'fixed', 'linear', or 'exponential'",
                ))
            }
        };

        let orchestrator = self.inner.lock().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Failed to acquire orchestrator lock")
        })?;

        orchestrator.set_retry_config(RetryConfig::new(max_attempts).with_backoff(strategy));
        Ok(())
    }

    /// Run a synthetic load test that exercises the same budget-enforcement
    /// and dynamic-routing logic `run()` uses under real traffic, at a
    /// volume/speed that would be impractical (and expensive) against live
    /// provider APIs. NOTE: request latencies/costs are simulated, not real
    /// network calls — this measures orchestration overhead and
    /// budget/routing behavior under load, not live provider performance.
    #[pyo3(signature = (num_requests=100, budget_usd=10.0))]
    pub fn run_load_test(
        &self,
        py: Python<'_>,
        num_requests: u32,
        budget_usd: f32,
    ) -> PyResult<PyObject> {
        py.allow_threads(|| {
            let config = RealLoadTestConfig {
                num_requests,
                budget_usd,
                ..Default::default()
            };
            let mut tester = RealLoadTester::new(config);
            let result = tester.run_load_test();

            Python::with_gil(|py| {
                let dict = PyDict::new_bound(py);
                dict.set_item("total_requests", result.total_requests)?;
                dict.set_item("successful_requests", result.successful_requests)?;
                dict.set_item("failed_requests", result.failed_requests)?;
                dict.set_item("total_cost_usd", result.total_cost_usd)?;
                dict.set_item("avg_latency_ms", result.avg_latency_ms)?;
                dict.set_item("min_latency_ms", result.min_latency_ms)?;
                dict.set_item("max_latency_ms", result.max_latency_ms)?;
                dict.set_item("p95_latency_ms", result.p95_latency_ms)?;
                dict.set_item("p99_latency_ms", result.p99_latency_ms)?;
                dict.set_item("requests_per_second", result.requests_per_second)?;
                dict.set_item("success_rate", result.success_rate)?;
                dict.set_item("budget_used_percent", result.budget_used_percent)?;
                dict.set_item("budget_alerts", result.budget_alerts)?;
                dict.set_item("dynamic_routing_changes", result.dynamic_routing_changes)?;
                Ok(dict.into())
            })
        })
    }

    pub fn profile_hardware(&self, py: Python<'_>) -> PyResult<PyHardwareProfile> {
        py.allow_threads(|| {
            let orchestrator = self.inner.lock().map_err(|_| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                    "Failed to acquire orchestrator lock",
                )
            })?;

            let profile = self
                .runtime
                .block_on(orchestrator.profile_hardware())
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

            Ok(PyHardwareProfile {
                total_memory_bytes: profile.total_memory_bytes,
                memory_tier: profile.memory_tier.to_string(),
                recommended_model_tier: profile.recommended_model_tier.to_string(),
                is_apple_silicon: profile.is_apple_silicon,
                has_metal: profile.has_metal,
                available_ollama_models: profile.available_ollama_models.clone(),
                best_available_model: profile.best_available_model.clone(),
            })
        })
    }

    pub fn available_backends(&self) -> PyResult<Vec<String>> {
        Ok(vec![
            "anthropic".to_string(),
            "openai".to_string(),
            "gemini".to_string(),
            "ollama".to_string(),
            "vllm".to_string(),
            "tensorrt_llm".to_string(),
            "mlc_llm".to_string(),
            "colibri".to_string(),
        ])
    }

    pub fn __repr__(&self) -> String {
        "Orchestrator(execution_mode=auto)".to_string()
    }
}

#[pyclass]
pub struct PyWorkloadResult {
    inner: pyinferencemanager_core::WorkloadResult,
}

#[pymethods]
impl PyWorkloadResult {
    #[getter]
    pub fn output(&self) -> String {
        self.inner.output.clone()
    }

    #[getter]
    pub fn total_tokens(&self) -> u32 {
        self.inner.total_tokens
    }

    #[getter]
    pub fn total_cost_usd(&self) -> f32 {
        self.inner.total_cost_usd
    }

    #[getter]
    pub fn total_latency_ms(&self) -> u64 {
        self.inner.total_latency_ms
    }

    #[getter]
    pub fn engines_used(&self) -> Vec<String> {
        self.inner.engines_used.clone()
    }

    #[getter]
    pub fn cache_hits(&self) -> u32 {
        self.inner.cache_hits
    }

    pub fn to_dict(&self, py: Python) -> PyResult<PyObject> {
        let dict = pyo3::types::PyDict::new_bound(py);
        dict.set_item("output", &self.inner.output)?;
        dict.set_item("total_tokens", self.inner.total_tokens)?;
        dict.set_item("total_cost_usd", self.inner.total_cost_usd)?;
        dict.set_item("total_latency_ms", self.inner.total_latency_ms)?;
        dict.set_item("engines_used", &self.inner.engines_used)?;
        dict.set_item("cache_hits", self.inner.cache_hits)?;
        Ok(dict.into())
    }
}

#[pyclass]
pub struct PyExecutionPlan {
    stages: u32,
    estimated_cost_usd: f32,
    estimated_latency_ms: u64,
    local_first: bool,
}

#[pymethods]
impl PyExecutionPlan {
    #[getter]
    pub fn stages(&self) -> u32 {
        self.stages
    }

    #[getter]
    pub fn estimated_cost_usd(&self) -> f32 {
        self.estimated_cost_usd
    }

    #[getter]
    pub fn estimated_latency_ms(&self) -> u64 {
        self.estimated_latency_ms
    }

    #[getter]
    pub fn local_first(&self) -> bool {
        self.local_first
    }
}

#[pymodule]
fn _core(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    // Core classes
    m.add_class::<PyOrchestrator>()?;
    m.add_class::<PyWorkloadResult>()?;
    m.add_class::<PyExecutionPlan>()?;

    // New infrastructure classes
    m.add_class::<PyBackendKind>()?;
    m.add_class::<PyHardwareProfile>()?;

    Ok(())
}
