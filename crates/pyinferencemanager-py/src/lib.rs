use pyo3::prelude::*;
use pyinferencemanager_core::{Orchestrator, OrchestratorConfig, ExecutionMode};
use std::sync::Arc;
use std::sync::Mutex;

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
            _ => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "mode must be 'local_first' or 'cloud_first'",
            )),
        };

        let config = OrchestratorConfig::default()
            .with_execution_mode(execution_mode);

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
        use pyinferencemanager_core::types::{Task, PrivacyLevel, Attachment, AttachmentKind};

        let privacy_level = match privacy {
            "high" => PrivacyLevel::High,
            "low" => PrivacyLevel::Low,
            _ => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "privacy must be 'high' or 'low'",
            )),
        };

        let mut py_task = Task::new(task.to_string())
            .with_options(
                pyinferencemanager_core::types::TaskOptions {
                    privacy: privacy_level,
                    ..Default::default()
                }
            );

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

        let orchestrator = self.inner.lock()
            .map_err(|_| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "Failed to acquire orchestrator lock"
            ))?;

        let result = self.runtime
            .block_on(orchestrator.execute(py_task))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        Ok(PyWorkloadResult { inner: result })
    }

    pub fn plan(&self, task: &str) -> PyResult<PyExecutionPlan> {
        use pyinferencemanager_core::types::Task;

        let py_task = Task::new(task.to_string());

        let orchestrator = self.inner.lock()
            .map_err(|_| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "Failed to acquire orchestrator lock"
            ))?;

        let plan = self.runtime
            .block_on(orchestrator.plan(&py_task))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        Ok(PyExecutionPlan {
            stages: plan.stages.len() as u32,
            estimated_cost_usd: plan.estimated_cost_usd,
            estimated_latency_ms: plan.estimated_latency_ms,
            local_first: plan.local_first,
        })
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
    m.add_class::<PyOrchestrator>()?;
    m.add_class::<PyWorkloadResult>()?;
    m.add_class::<PyExecutionPlan>()?;
    Ok(())
}
