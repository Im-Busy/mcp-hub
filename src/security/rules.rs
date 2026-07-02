//! Categorized security rules with trust scoring.
//!
//! Each security rule belongs to a `RuleCategory` and has a `Severity` level.
//! When rules trigger, a `TrustScore` (0–100) is computed from the triggered
//! categories. Repeated same-category triggers incur a small additional deduction.
//! The score maps to three `TrustTier` levels for decision-making.

use serde::{Deserialize, Serialize};

/// Categories of security risk that a rule can detect.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RuleCategory {
    PathTraversal,
    CommandInjection,
    FileExfiltration,
    CredentialLeak,
    NetworkAccess,
    ResourceExhaustion,
    DataTampering,
}

impl std::fmt::Display for RuleCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            RuleCategory::PathTraversal => "path-traversal",
            RuleCategory::CommandInjection => "command-injection",
            RuleCategory::FileExfiltration => "file-exfiltration",
            RuleCategory::CredentialLeak => "credential-leak",
            RuleCategory::NetworkAccess => "network-access",
            RuleCategory::ResourceExhaustion => "resource-exhaustion",
            RuleCategory::DataTampering => "data-tampering",
        };
        write!(f, "{}", s)
    }
}

/// Severity level for security rules with numeric trust score deductions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Low = 3,
    Medium = 8,
    High = 15,
    Critical = 25,
}

/// A categorized security rule for the rule engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorizedSecurityRule {
    pub name: String,
    pub pattern: String,
    pub block_message: String,
    pub enabled: bool,
    pub category: RuleCategory,
    pub severity: Severity,
}

/// Trust score (0–100) computed from triggered rule categories.
#[derive(Debug, Clone, Copy)]
pub struct TrustScore(pub u16);

/// Computes trust score: 100 minus sum of unique rule severity deductions.
/// Repeated same-category triggers cost 1 additional point each.
pub fn compute_trust_score(triggered: &[RuleCategory]) -> TrustScore {
    let mut seen = std::collections::HashSet::new();
    let mut deduction: u16 = 0;
    for category in triggered {
        if seen.insert(category.clone()) {
            deduction += category_severity_deduction(category);
        } else {
            deduction += 1;
        }
    }
    let score = 100u16.saturating_sub(deduction);
    TrustScore(score)
}

fn category_severity_deduction(category: &RuleCategory) -> u16 {
    match category {
        RuleCategory::PathTraversal => 15,
        RuleCategory::CommandInjection => 25,
        RuleCategory::FileExfiltration => 15,
        RuleCategory::CredentialLeak => 25,
        RuleCategory::NetworkAccess => 8,
        RuleCategory::ResourceExhaustion => 8,
        RuleCategory::DataTampering => 15,
    }
}

/// Trust level tiers derived from trust score.
pub enum TrustTier {
    /// 85–100: No significant security incidents.
    Safe,
    /// 50–84: Some security rule triggers, monitor closely.
    AtRisk,
    /// 0–49: Significant security concerns, consider blocking.
    HighRisk,
}

impl TrustScore {
    pub fn tier(&self) -> TrustTier {
        match self.0 {
            85..=100 => TrustTier::Safe,
            50..=84 => TrustTier::AtRisk,
            _ => TrustTier::HighRisk,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trust_score_perfect() {
        let score = compute_trust_score(&[]);
        assert_eq!(score.0, 100);
    }

    #[test]
    fn test_trust_score_single_critical() {
        let score = compute_trust_score(&[RuleCategory::CredentialLeak]);
        assert_eq!(score.0, 75);
    }

    #[test]
    fn test_trust_score_repeated_low() {
        let score = compute_trust_score(&[
            RuleCategory::NetworkAccess,
            RuleCategory::NetworkAccess,
            RuleCategory::NetworkAccess,
        ]);
        // 100 - 8 (first unique) - 1 (second repeat) - 1 (third repeat) = 90
        assert_eq!(score.0, 90);
    }

    #[test]
    fn test_trust_tier_boundaries() {
        assert!(matches!(TrustScore(100).tier(), TrustTier::Safe));
        assert!(matches!(TrustScore(85).tier(), TrustTier::Safe));
        assert!(matches!(TrustScore(84).tier(), TrustTier::AtRisk));
        assert!(matches!(TrustScore(50).tier(), TrustTier::AtRisk));
        assert!(matches!(TrustScore(49).tier(), TrustTier::HighRisk));
        assert!(matches!(TrustScore(0).tier(), TrustTier::HighRisk));
    }
}
