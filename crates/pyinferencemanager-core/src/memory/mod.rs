pub mod resource_monitor;

pub use resource_monitor::{ResourceMonitor, ResourceSnapshot, VramInfo};

pub struct HierarchicalMemoryEngine {
    resource_monitor: ResourceMonitor,
}

impl HierarchicalMemoryEngine {
    pub fn new() -> Self {
        HierarchicalMemoryEngine {
            resource_monitor: ResourceMonitor,
        }
    }

    pub async fn snapshot(&self, total_memory: u64) -> crate::Result<ResourceSnapshot> {
        ResourceMonitor::snapshot(total_memory).await
    }
}

impl Default for HierarchicalMemoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hierarchical_memory_engine_new() {
        let _engine = HierarchicalMemoryEngine::new();
        assert!(ResourceMonitor::cpu_core_count() > 0);
    }
}
