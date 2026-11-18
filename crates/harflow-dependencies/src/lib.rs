use harflow_model::ConfirmationStatus;
use serde::{Deserialize, Serialize};

/// Five confidence factors specified in SRS Section 4:
/// `C = 0.30*V + 0.20*T + 0.20*U + 0.20*R + 0.10*S`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceFactors {
    pub value_match: f64,
    pub temporal_order: f64,
    pub uniqueness: f64,
    pub cross_session_repeatability: f64,
    pub structural_compatibility: f64,
}

/// Computes the deterministic dependency confidence score, enforcing mandatory temporal precedence.
#[must_use]
pub fn score_confidence(factors: &ConfidenceFactors) -> f64 {
    if factors.temporal_order <= 0.0 {
        return 0.0;
    }

    let v = factors.value_match.clamp(0.0, 1.0);
    let t = factors.temporal_order.clamp(0.0, 1.0);
    let u = factors.uniqueness.clamp(0.0, 1.0);
    let r = factors.cross_session_repeatability.clamp(0.0, 1.0);
    let s = factors.structural_compatibility.clamp(0.0, 1.0);

    let raw = 0.30 * v + 0.20 * t + 0.20 * u + 0.20 * r + 0.10 * s;
    (raw * 10_000.0).round() / 10_000.0
}

/// Maps a calibrated score to its initial automated confirmation status.
#[must_use]
pub fn classify_confidence(score: f64) -> ConfirmationStatus {
    if score >= 0.90 {
        ConfirmationStatus::Confirmed
    } else if score >= 0.65 {
        ConfirmationStatus::Likely
    } else {
        ConfirmationStatus::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_exact_weighted_confidence_and_enforces_temporal_gate() {
        let high = ConfidenceFactors {
            value_match: 1.0,
            temporal_order: 1.0,
            uniqueness: 1.0,
            cross_session_repeatability: 0.8,
            structural_compatibility: 1.0,
        };
        let score = score_confidence(&high);
        assert!((score - 0.96).abs() < 1e-6);
        assert_eq!(classify_confidence(score), ConfirmationStatus::Confirmed);

        let likely = ConfidenceFactors {
            value_match: 1.0,
            temporal_order: 1.0,
            uniqueness: 0.5,
            cross_session_repeatability: 0.5,
            structural_compatibility: 0.5,
        };
        assert_eq!(
            classify_confidence(score_confidence(&likely)),
            ConfirmationStatus::Likely
        );

        let invalid_temporal = ConfidenceFactors {
            temporal_order: 0.0,
            ..high
        };
        assert!((score_confidence(&invalid_temporal) - 0.0).abs() < 1e-9);
        assert_eq!(
            classify_confidence(score_confidence(&invalid_temporal)),
            ConfirmationStatus::Unknown
        );
    }
}
