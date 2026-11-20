//! Experimental hybrid decision policy. No filesystem access or image modification.
//!
//! This module does NOT implement ORB: its evidence must come from an independently
//! validated feature extractor. Production similarity scanning remains pHash-only.
//! See docs/PhotoVault_evaluacion_hibrida.md.
#![allow(dead_code)]

/// The experimental threshold is provisional, not a calibrated probability.
pub const DEFAULT_MIN_ORB_INLIERS: u32 = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceKind {
    PerceptualHash,
    OrbGeometry,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbEvidence {
    pub mutual_matches: u32,
    pub ransac_inliers: u32,
    pub inlier_ratio: f32,
    /// True only if the independent geometric-spread validation succeeded.
    pub spatially_distributed: bool,
}

impl OrbEvidence {
    pub fn valid(self) -> bool {
        self.mutual_matches >= 12
            && self.ransac_inliers >= 12
            && self.ransac_inliers <= self.mutual_matches
            && self.inlier_ratio.is_finite()
            && (0.0..=1.0).contains(&self.inlier_ratio)
            && self.inlier_ratio >= 0.35
            && self.spatially_distributed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HybridPolicy {
    pub min_orb_inliers: u32,
}

impl Default for HybridPolicy {
    fn default() -> Self {
        Self { min_orb_inliers: DEFAULT_MIN_ORB_INLIERS }
    }
}

impl HybridPolicy {
    /// Return *suggestion evidence*, never a declaration of exact duplicates.
    /// A successful pHash comparison is not invalidated by absent ORB evidence.
    pub fn classify(self, phash_similar: bool, orb: Option<OrbEvidence>) -> Option<EvidenceKind> {
        let orb_similar = orb.is_some_and(|e| {
            e.valid() && e.ransac_inliers >= self.min_orb_inliers.max(12)
        });
        match (phash_similar, orb_similar) {
            (false, false) => None,
            (true, false) => Some(EvidenceKind::PerceptualHash),
            (false, true) => Some(EvidenceKind::OrbGeometry),
            (true, true) => Some(EvidenceKind::Both),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(inliers: u32) -> OrbEvidence {
        OrbEvidence {
            mutual_matches: 50,
            ransac_inliers: inliers,
            inlier_ratio: inliers as f32 / 50.0,
            spatially_distributed: true,
        }
    }

    #[test]
    fn no_evidence_means_no_suggestion() {
        assert_eq!(HybridPolicy::default().classify(false, None), None);
    }

    #[test]
    fn phash_still_works_without_orb() {
        assert_eq!(HybridPolicy::default().classify(true, None), Some(EvidenceKind::PerceptualHash));
    }

    #[test]
    fn orb_only_requires_threshold_and_geometry() {
        let policy = HybridPolicy::default();
        assert_eq!(policy.classify(false, Some(evidence(24))), None);
        assert_eq!(policy.classify(false, Some(evidence(25))), Some(EvidenceKind::OrbGeometry));
    }

    #[test]
    fn both_sources_are_distinguished() {
        assert_eq!(HybridPolicy::default().classify(true, Some(evidence(25))), Some(EvidenceKind::Both));
    }

    #[test]
    fn invalid_geometry_never_adds_an_orb_match() {
        let policy = HybridPolicy::default();
        let mut e = evidence(25);
        e.spatially_distributed = false;
        assert_eq!(policy.classify(false, Some(e)), None);
        e.spatially_distributed = true;
        e.inlier_ratio = f32::NAN;
        assert_eq!(policy.classify(false, Some(e)), None);
        e.inlier_ratio = 1.2;
        assert_eq!(policy.classify(false, Some(e)), None);
        e.inlier_ratio = 0.1;
        assert_eq!(policy.classify(false, Some(e)), None);
        e = evidence(25);
        e.mutual_matches = 20;
        assert_eq!(policy.classify(false, Some(e)), None);
    }

    #[test]
    fn threshold_is_configurable_but_never_below_geometry_minimum() {
        let policy = HybridPolicy { min_orb_inliers: 0 };
        assert_eq!(policy.classify(false, Some(evidence(11))), None);
        assert_eq!(policy.classify(false, Some(evidence(12))), None); // ratio < 0.35
        assert_eq!(policy.classify(false, Some(evidence(18))), Some(EvidenceKind::OrbGeometry));
    }
}
