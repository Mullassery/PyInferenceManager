use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Budget configuration for cost control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub max_cost_usd: f32,
    pub max_requests: u32,
    pub alert_threshold_percent: f32,  // Alert at 80% of budget
    pub enforce_hard_limit: bool,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_cost_usd: 100.0,
            max_requests: 1000,
            alert_threshold_percent: 80.0,
            enforce_hard_limit: true,
        }
    }
}

/// Tracks budget usage and enforces limits
#[derive(Debug, Clone)]
pub struct BudgetEnforcer {
    config: BudgetConfig,
    current_cost: Arc<parking_lot::Mutex<f32>>,
    request_count: Arc<AtomicU64>,
    alerts: Arc<parking_lot::Mutex<Vec<BudgetAlert>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetAlert {
    pub alert_type: String,
    pub message: String,
    pub timestamp: i64,
    pub current_cost_usd: f32,
    pub percent_used: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub current_cost_usd: f32,
    pub max_cost_usd: f32,
    pub percent_used: f32,
    pub remaining_budget_usd: f32,
    pub current_requests: u64,
    pub max_requests: u32,
    pub within_budget: bool,
    pub alerts: Vec<BudgetAlert>,
}

impl BudgetEnforcer {
    pub fn new(config: BudgetConfig) -> Self {
        Self {
            config,
            current_cost: Arc::new(parking_lot::Mutex::new(0.0)),
            request_count: Arc::new(AtomicU64::new(0)),
            alerts: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }

    pub fn record_cost(&self, cost_usd: f32) -> Result<(), String> {
        let mut current = self.current_cost.lock();
        *current += cost_usd;

        let percent_used = (*current / self.config.max_cost_usd) * 100.0;

        // Check hard limit
        if self.config.enforce_hard_limit && *current > self.config.max_cost_usd as f32 {
            let msg = format!(
                "Budget exceeded: ${:.4} / ${:.4}",
                current, self.config.max_cost_usd
            );
            self.add_alert("budget_exceeded".to_string(), msg.clone());
            return Err(msg);
        }

        // Check alert threshold
        if percent_used >= self.config.alert_threshold_percent {
            let msg = format!(
                "Budget alert: {:.1}% used (${:.4} / ${:.4})",
                percent_used, current, self.config.max_cost_usd
            );
            self.add_alert("budget_warning".to_string(), msg);
        }

        self.request_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub fn can_execute(&self) -> bool {
        let current = *self.current_cost.lock();
        let requests = self.request_count.load(Ordering::Relaxed);

        !self.config.enforce_hard_limit
            || (current < self.config.max_cost_usd
                && requests < self.config.max_requests as u64)
    }

    pub fn get_status(&self) -> BudgetStatus {
        let current_cost = *self.current_cost.lock();
        let requests = self.request_count.load(Ordering::Relaxed);
        let percent_used = (current_cost / self.config.max_cost_usd) * 100.0;

        BudgetStatus {
            current_cost_usd: current_cost,
            max_cost_usd: self.config.max_cost_usd,
            percent_used,
            remaining_budget_usd: (self.config.max_cost_usd - current_cost).max(0.0),
            current_requests: requests,
            max_requests: self.config.max_requests,
            within_budget: current_cost <= self.config.max_cost_usd,
            alerts: self.alerts.lock().clone(),
        }
    }

    pub fn reset_budget(&self) {
        *self.current_cost.lock() = 0.0;
        self.request_count.store(0, Ordering::Relaxed);
        self.alerts.lock().clear();
    }

    fn add_alert(&self, alert_type: String, message: String) {
        let current_cost = *self.current_cost.lock();
        let percent_used = (current_cost / self.config.max_cost_usd) * 100.0;

        let alert = BudgetAlert {
            alert_type,
            message,
            timestamp: chrono::Utc::now().timestamp_millis(),
            current_cost_usd: current_cost,
            percent_used,
        };

        self.alerts.lock().push(alert);
    }

    pub fn get_alerts(&self) -> Vec<BudgetAlert> {
        self.alerts.lock().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_config_default() {
        let config = BudgetConfig::default();
        assert_eq!(config.max_cost_usd, 100.0);
        assert_eq!(config.max_requests, 1000);
        assert!(config.enforce_hard_limit);
    }

    #[test]
    fn test_budget_enforcer_new() {
        let config = BudgetConfig::default();
        let enforcer = BudgetEnforcer::new(config);
        let status = enforcer.get_status();
        assert_eq!(status.current_cost_usd, 0.0);
        assert_eq!(status.current_requests, 0);
    }

    #[test]
    fn test_budget_enforcer_record_cost() {
        let config = BudgetConfig::default();
        let enforcer = BudgetEnforcer::new(config);
        let result = enforcer.record_cost(25.0);
        assert!(result.is_ok());
        assert_eq!(enforcer.get_status().current_cost_usd, 25.0);
    }

    #[test]
    fn test_budget_enforcer_hard_limit() {
        let config = BudgetConfig {
            max_cost_usd: 50.0,
            enforce_hard_limit: true,
            ..Default::default()
        };
        let enforcer = BudgetEnforcer::new(config);
        let result1 = enforcer.record_cost(40.0);
        assert!(result1.is_ok());
        let result2 = enforcer.record_cost(20.0);
        assert!(result2.is_err());
    }

    #[test]
    fn test_budget_enforcer_alert_threshold() {
        let config = BudgetConfig {
            max_cost_usd: 100.0,
            alert_threshold_percent: 75.0,
            enforce_hard_limit: false,
            ..Default::default()
        };
        let enforcer = BudgetEnforcer::new(config);
        enforcer.record_cost(80.0).ok();
        let alerts = enforcer.get_alerts();
        assert!(!alerts.is_empty());
    }

    #[test]
    fn test_budget_enforcer_can_execute() {
        let config = BudgetConfig::default();
        let enforcer = BudgetEnforcer::new(config);
        assert!(enforcer.can_execute());
        enforcer.record_cost(150.0).ok();
        assert!(!enforcer.can_execute());
    }

    #[test]
    fn test_budget_enforcer_reset() {
        let config = BudgetConfig::default();
        let enforcer = BudgetEnforcer::new(config);
        enforcer.record_cost(25.0).ok();
        enforcer.reset_budget();
        assert_eq!(enforcer.get_status().current_cost_usd, 0.0);
        assert_eq!(enforcer.get_status().current_requests, 0);
    }

    #[test]
    fn test_budget_status() {
        let config = BudgetConfig {
            max_cost_usd: 100.0,
            ..Default::default()
        };
        let enforcer = BudgetEnforcer::new(config);
        enforcer.record_cost(50.0).ok();
        let status = enforcer.get_status();
        assert_eq!(status.percent_used, 50.0);
        assert_eq!(status.remaining_budget_usd, 50.0);
        assert!(status.within_budget);
    }
}
