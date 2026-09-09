//! Model for cat qubits.
//!
//! Cat qubits are characterized by:
//! - the physical error rate κ₁/κ₂
//! - their average number of photons |α|²
//!
//! Default value (and only one compatible with the magic state factories
//! precomputations) is κ₁/κ₂=1e-5, while |α|² is considered as an error
//! correction code parameter and not handled in this module (resource estimator
//! will optimized on it).

/// Struct for cat qubits, stores κ₁/κ₂, the ratio between the one and two
/// photon loss rates, as it defines the intrinsic physical error rate.
#[must_use]
pub struct CatQubit {
    pub(crate) k1_k2: f64,
}

impl Default for CatQubit {
    /// Set κ₁/κ₂ to a default value of 1e-5, as in
    /// [arXiv:2302.06639](https://arxiv.org/abs/2302.06639) (p. 2).
    fn default() -> Self {
        Self { k1_k2: 1e-5 }
    }
}

impl CatQubit {
    /// Instantiation from the default value κ₁/κ₂ = 1e-5.
    pub fn new() -> Self {
        // κ₁/κ₂ hard-coded in the factories performances. Think twice before
        // changing this.
        Self::default()
    }

    /// Instantiation from an explicit κ₁/κ₂ value.
    pub fn with_k1_k2(k1_k2: f64) -> Self {
        Self { k1_k2 }
    }
}
