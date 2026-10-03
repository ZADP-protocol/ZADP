use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PolicyDecision {
    Allow,
    Deny,
    RequireApproval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub max_single_payment_zec: Decimal,
    pub max_daily_payment_zec: Decimal,
    pub require_human_approval_above_zec: Decimal,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            max_single_payment_zec: Decimal::new(5, 2), // 0.05
            max_daily_payment_zec: Decimal::new(20, 2), // 0.20
            require_human_approval_above_zec: Decimal::new(3, 2), // 0.03
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpendingPolicy {
    config: PolicyConfig,
    daily_total: Decimal,
    day: chrono::NaiveDate,
}

impl SpendingPolicy {
    pub fn new(config: PolicyConfig) -> Self {
        Self {
            config,
            daily_total: Decimal::ZERO,
            day: Utc::now().date_naive(),
        }
    }

    pub fn config(&self) -> &PolicyConfig {
        &self.config
    }

    pub fn evaluate(&mut self, amount: Decimal, now: DateTime<Utc>) -> PolicyDecision {
        self.roll_day(now);
        if amount <= Decimal::ZERO {
            return PolicyDecision::Deny;
        }
        if amount > self.config.max_single_payment_zec {
            return PolicyDecision::Deny;
        }
        if self.daily_total + amount > self.config.max_daily_payment_zec {
            return PolicyDecision::Deny;
        }
        if amount > self.config.require_human_approval_above_zec {
            return PolicyDecision::RequireApproval;
        }
        PolicyDecision::Allow
    }

    pub fn record_spend(&mut self, amount: Decimal, now: DateTime<Utc>) {
        self.roll_day(now);
        self.daily_total += amount;
    }

    fn roll_day(&mut self, now: DateTime<Utc>) {
        let today = now.date_naive();
        if today != self.day {
            self.day = today;
            self.daily_total = Decimal::ZERO;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_small_payment() {
        let mut p = SpendingPolicy::new(PolicyConfig::default());
        assert_eq!(
            p.evaluate(Decimal::new(1, 2), Utc::now()),
            PolicyDecision::Allow
        );
    }

    #[test]
    fn requires_approval_above_threshold() {
        let mut p = SpendingPolicy::new(PolicyConfig::default());
        assert_eq!(
            p.evaluate(Decimal::new(4, 2), Utc::now()),
            PolicyDecision::RequireApproval
        );
    }

    #[test]
    fn denies_over_single_max() {
        let mut p = SpendingPolicy::new(PolicyConfig::default());
        assert_eq!(
            p.evaluate(Decimal::new(6, 2), Utc::now()),
            PolicyDecision::Deny
        );
    }
}
