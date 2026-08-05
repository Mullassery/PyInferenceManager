use pyinferencemanager_core::{ExecutionMode, Orchestrator, OrchestratorConfig, backends::BackendKind};
use pyo3::prelude::*;
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
    fn new(mode: &str) -> PyResult<Self> {
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

        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        let orchestrator = runtime
            .block_on(Orchestrator::new(config))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        Ok(PyOrchestrator {
            inner: Arc::new(Mutex::new(orchestrator)),
            runtime,
        })
    }

    #[pyo3(signature = (task, file=None, message=None, privacy="low"))]
    pub fn run(
        &self,
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

        let mut py_task =
            Task::new(task.to_string()).with_options(pyinferencemanager_core::types::TaskOptions {
                privacy: privacy_level,
                ..Default::default()
            });

        if let Some(file_path) = file {
            if let Ok(content) = std::fs::read(file_path) {
                let attachment = Attachment {
                    kind: AttachmentKind::File,
                    content,
                    mime_type: "application/octet-stream".to_string(),
                    name: file_path.to_string(),
                };
                py_task = py_task.with_attachment(attachment);
            }
        }

        if let Some(msg) = message {
            let attachment = Attachment {
                kind: AttachmentKind::RawText,
                content: msg.as_bytes().to_vec(),
                mime_type: "text/plain".to_string(),
                name: "message".to_string(),
            };
            py_task = py_task.with_attachment(attachment);
        }

        let orchestrator = self.inner.lock().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Failed to acquire orchestrator lock")
        })?;

        let result = self
            .runtime
            .block_on(orchestrator.execute(py_task))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        Ok(PyWorkloadResult { inner: result })
    }

    pub fn plan(&self, task: &str) -> PyResult<PyExecutionPlan> {
        use pyinferencemanager_core::types::Task;

        let py_task = Task::new(task.to_string());

        let orchestrator = self.inner.lock().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Failed to acquire orchestrator lock")
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
    }

    pub fn provider_ranking(&self) -> PyResult<Vec<(String, f32)>> {
        let orchestrator = self.inner.lock().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Failed to acquire orchestrator lock")
        })?;

        Ok(orchestrator.provider_ranking())
    }

    pub fn profile_hardware(&self) -> PyResult<PyHardwareProfile> {
        let orchestrator = self.inner.lock().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Failed to acquire orchestrator lock")
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
    }

    pub fn available_backends(&self) -> PyResult<Vec<String>> {
        Ok(vec![
            "anthropic".to_string(),
            "openai".to_string(),
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
