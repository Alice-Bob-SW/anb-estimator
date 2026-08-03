//! Repetition code for biased error correction with a focus on phase flips.
//!
//! The code and its performances are described in
//! [arXiv:2302.06639](https://arxiv.org/abs/2302.06639).
//!
//! Code parameters:
//! - code distance
//! - average number of photons |α|²
//! - k1/k2
//!
//! Hard-coded values:
//! - max distance (for iteration) = 49
//! - max |α|² (for iteration) = 30.0

use num_traits::{FromPrimitive, ToPrimitive};
use std::fmt::Display;

use resource_estimator::estimates::ErrorCorrection;

use crate::hardware::Hardware;
use crate::logical_utils;
use crate::qubit::CatQubit;

/// How κ₁/κ₂ is handled during parameter search.
#[derive(Debug, Clone)]
pub enum K1K2Spec {
    /// Do not optimize over κ₁/κ₂; always use this value.
    Fixed(f64),
    /// Optimize over κ₁/κ₂ using an explicit list of values.
    Values(Vec<f64>),
}

/// Chose whether to optimize for energy or qubits.
#[derive(Debug, Clone, Copy)]
pub enum OptimizationTarget {
    /// Optimize for energy
    Energy,
    /// Optimize for physical qubits
    Qubits,
}

/// Represents a repetition code.
pub struct RepetitionCode {
    p_threshold: f64,
    optimization_target: OptimizationTarget,
    k1_k2_spec: K1K2Spec,
}

impl RepetitionCode {
    /// Default initialization, with threshold at 0.013.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a repetition code that optimizes the *energy* cost.
    #[must_use]
    pub fn new_energy_optimized() -> Self {
        Self {
            optimization_target: OptimizationTarget::Energy,
            ..Self::default()
        }
    }

    /// Build a repetition code that explicitly optimizes *qubits*.
    #[must_use]
    pub fn new_qubit_optimized() -> Self {
        Self {
            optimization_target: OptimizationTarget::Qubits,
            ..Self::default()
        }
    }

    /// Change the optimization mode on an existing instance (must be done
    /// before passing it into the estimator).
    pub fn set_optimization_target(&mut self, target: OptimizationTarget) {
        self.optimization_target = target;
    }

    /// Option 2: user provides a single κ₁/κ₂ value -> fixed, not optimized over.
    pub fn set_k1_k2(&mut self, k1_k2: f64) {
        assert!(
            k1_k2.is_finite() && k1_k2 > 0.0,
            "κ₁/κ₂ must be finite and > 0"
        );
        self.k1_k2_spec = K1K2Spec::Fixed(k1_k2);
    }

    /// Option 3: explicit κ₁/κ₂ values (optimized over).
    pub fn set_k1_k2_values(&mut self, values: Vec<f64>) {
        assert!(!values.is_empty(), "κ₁/κ₂ values must not be empty");
        assert!(
            values.iter().all(|&k| k.is_finite() && k > 0.0),
            "all κ₁/κ₂ values must be finite and > 0"
        );
        self.k1_k2_spec = K1K2Spec::Values(values);
    }

    #[must_use]
    /// Logical phaseflip probability per round, as given by
    /// [arXiv:2302.06639](https://arxiv.org/abs/2302.06639) (p. 28, eq. E1).
    fn logical_phaseflip_probability(
        &self,
        _physical_qubit: &CatQubit,
        parameter: &CodeParameter,
    ) -> Option<f64> {
        // arXiv:2302.06639 (p. 29, Fig. 26)
        let prefactor = 5.6e-2;
        let exponent = (i32::from_u64(parameter.distance)? + 1) / 2;

        // Logical phase-flip error rate per cycle of the repetition code
        // arXiv:2302.06639 (p. 3, eq. 4)
        Some(
            prefactor
                * ((parameter.alpha_sq.powf(0.86) * parameter.k1_k2) / self.p_threshold)
                    .powi(exponent),
        )
    }

    #[allow(clippy::similar_names)]
    #[must_use]
    /// Logical bitflip probability per round, as given in
    /// [arXiv:2302.06639](https://arxiv.org/abs/2302.06639) (eq. 3).
    fn logical_bitflip_probability(parameter: &CodeParameter) -> Option<f64> {
        // number of CX gates in a repetition code cycle
        let ncx = 2 * (parameter.distance - 1);

        // Bit-flip error probability of a CX gate (numerically estimated using
        // full process tomography), arXiv:2302.06639 (p. 26, eq. D8)
        let pcx = 0.5 * (-2.0 * parameter.alpha_sq).exp();

        Some(f64::from_u64(ncx)? * pcx)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn energy_per_round(parameter: &CodeParameter, _macro_flag: bool) -> f64 {
        // ratio κ₁/κ₂ now comes from the code parameter
        let k1_on_k2 = parameter.k1_k2;

        // Build a Hardware object with the right α
        let mut hw = Hardware {
            alpha: parameter.alpha_sq.sqrt(),
            ..Hardware::default()
        };

        let dist = parameter.distance as usize;
        let num_round = 1usize;

        logical_utils::e_tot(k1_on_k2, &mut hw, dist, num_round, true)
    }
}

impl Default for RepetitionCode {
    fn default() -> Self {
        let p_threshold = 0.013;

        // Option 1 default: κ₁/κ₂ fixed to 1e-5, not optimized over.
        let k1_k2_spec = K1K2Spec::Fixed(1.0e-5);

        Self {
            p_threshold,
            optimization_target: OptimizationTarget::Qubits,
            k1_k2_spec,
        }
    }
}

#[derive(Clone, PartialEq)]
/// Store the code distance, average photon number |α|² and κ₁/κ₂.
pub struct CodeParameter {
    pub(crate) distance: u64,
    // Amplitude ɑ arXiv:2302.06639 (p. 3), average number of photons |ɑ|²
    pub(crate) alpha_sq: f64,
    // Ratio κ₁/κ₂ used by the code.
    pub(crate) k1_k2: f64,
}

impl CodeParameter {
    #[must_use]
    /// Set new values for the code parameters (distance, |α|²).
    /// Uses a default κ₁/κ₂ = 1e-5 for backwards-compatibility.
    pub fn new(distance: u64, alpha_sq: f64) -> Self {
        Self::with_k1_k2(distance, alpha_sq, 1.0e-5)
    }

    #[must_use]
    /// Set new values for the code parameters (distance, |α|², κ₁/κ₂).
    pub fn with_k1_k2(distance: u64, alpha_sq: f64, k1_k2: f64) -> Self {
        Self {
            distance,
            alpha_sq,
            k1_k2,
        }
    }
}

impl Display for CodeParameter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (|ɑ|² = {}, κ₁/κ₂ = {})",
            self.distance, self.alpha_sq, self.k1_k2
        )
    }
}

/// Keeps the range of parameters on which to iterate.
struct CodeParameterRange {
    distance: u64,
    alpha_sq: u64,

    // κ₁/κ₂ handling
    k1_k2_spec: K1K2Spec,
    k_index: u32,
    k_count: u32,

    max_distance: u64,
    max_alpha_sq: u64,
}

impl CodeParameterRange {
    #[allow(clippy::cast_possible_truncation)]
    pub fn new(
        _lower_bound: Option<&CodeParameter>,
        max_distance: u64,
        max_alpha_sq: f64,
        k1_k2_spec: K1K2Spec,
    ) -> Self {
        let max_alpha_sq = max_alpha_sq
            .to_u64()
            .expect("max_alpha_sq failed to be represented as u64");

        let k_count = match &k1_k2_spec {
            K1K2Spec::Fixed(k) => {
                assert!(k.is_finite() && *k > 0.0, "κ₁/κ₂ must be finite and > 0");
                1
            }
            K1K2Spec::Values(values) => {
                assert!(!values.is_empty(), "κ₁/κ₂ values must not be empty");
                assert!(
                    values.iter().all(|&k| k.is_finite() && k > 0.0),
                    "all κ₁/κ₂ values must be finite and > 0"
                );
                values.len() as u32
            }
        };

        Self {
            distance: 1,
            alpha_sq: 1,
            k1_k2_spec,
            k_index: 0,
            k_count,
            max_distance,
            max_alpha_sq,
        }
    }

    fn compute_k1_k2(&self) -> Option<f64> {
        if self.k_index >= self.k_count {
            return None;
        }

        match &self.k1_k2_spec {
            K1K2Spec::Fixed(k) => Some(*k),
            K1K2Spec::Values(values) => values.get(self.k_index as usize).copied(),
        }
    }
}

impl Iterator for CodeParameterRange {
    type Item = CodeParameter;

    fn next(&mut self) -> Option<Self::Item> {
        let k1_k2 = self.compute_k1_k2()?;

        let result = CodeParameter::with_k1_k2(
            self.distance,
            self.alpha_sq.to_f64().expect("alpha_sq doesn't fit in f64"),
            k1_k2,
        );

        // Advance inner coordinates: α², then distance.
        if self.alpha_sq == self.max_alpha_sq {
            self.alpha_sq = 1;
            self.distance += 2; // keep distance odd
        } else {
            self.alpha_sq += 1;
        }

        // If we've exhausted all distances for this κ point, move to next κ.
        if self.distance > self.max_distance {
            self.distance = 1;
            self.alpha_sq = 1;
            self.k_index += 1;
        }

        Some(result)
    }
}

impl ErrorCorrection for RepetitionCode {
    type Qubit = CatQubit;
    type Parameter = CodeParameter;

    fn code_parameter_range(
        &self,
        lower_bound: Option<&Self::Parameter>,
    ) -> impl Iterator<Item = Self::Parameter> {
        CodeParameterRange::new(lower_bound, 49, 30.0, self.k1_k2_spec.clone())
    }

    fn physical_qubits(&self, parameter: &Self::Parameter) -> Result<u64, String> {
        // arXiv:2302.06639 (p. 27)
        Ok(2 * parameter.distance - 1)
    }

    fn logical_qubits(&self, _parameter: &Self::Parameter) -> Result<u64, String> {
        Ok(1)
    }

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    fn logical_cycle_time(
        &self,
        _qubit: &Self::Qubit,
        parameter: &Self::Parameter,
    ) -> Result<u64, String> {
        // Build the same Hardware model we use for energies
        let k1_on_k2 = parameter.k1_k2;

        let mut hw = Hardware {
            alpha: parameter.alpha_sq.sqrt(),
            ..Hardware::default()
        };

        // In logical_utils we do: k2 = hw.k_1 / k1_on_k2
        let k2 = hw.k_1 / k1_on_k2;

        // Duration for *one unit cell* of the repetition code
        let [_t_prep, _t_cnot, _t_meas, t_cycle_cell] =
            crate::logical_utils::duration_cycle(k2, &mut hw);

        let t_cycle_total_s = t_cycle_cell;

        // Convert seconds → nanoseconds and return as u64
        let t_cycle_ns = (t_cycle_total_s / 1.0e-9).round() as u64;
        Ok(t_cycle_ns)
    }

    fn logical_error_rate(
        &self,
        qubit: &Self::Qubit,
        parameter: &Self::Parameter,
    ) -> Result<f64, String> {
        if let (Some(code_distance_f64), Some(lzp), Some(lxp)) = (
            f64::from_u64(parameter.distance),
            self.logical_phaseflip_probability(qubit, parameter),
            Self::logical_bitflip_probability(parameter),
        ) {
            // arXiv:2302.06639 (p. 4, eq. 3 and app E2, p. 27)
            // this is eq. 3 in a more compact form
            Ok(code_distance_f64 * (lzp + lxp)) // First: logical phase-flip, second part: logical bit-flip
        } else {
            Err("cannot compute logical failure probability".into())
        }
    }

    fn compute_code_parameter(
        &self,
        qubit: &Self::Qubit,
        required_logical_error_rate: f64,
    ) -> Result<Self::Parameter, String> {
        use std::cmp::Ordering;

        let mut best: Option<Self::Parameter> = None;

        // Iterate over the full parameter range
        for param in self.code_parameter_range(None) {
            // Skip parameters that don't meet the logical error requirement
            let err = self.logical_error_rate(qubit, &param)?;
            if err > required_logical_error_rate {
                continue;
            }

            match &best {
                None => {
                    // First valid candidate
                    best = Some(param);
                }
                Some(current) => {
                    // Use our comparator (energy or qubits) to pick the better one
                    let ord = self.code_parameter_cmp(qubit, &param, current);
                    if ord == Ordering::Less {
                        best = Some(param);
                    }
                }
            }
        }

        best.ok_or_else(|| {
            "No code parameter satisfies the required logical error rate".to_string()
        })
    }

    fn code_parameter_cmp(
        &self,
        qubit: &Self::Qubit,
        p1: &Self::Parameter,
        p2: &Self::Parameter,
    ) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match self.optimization_target {
            OptimizationTarget::Energy => {
                // --- energy-optimized comparator ---

                // Primary cost: total energy per logical round (macro_flag = true)
                let e1 = Self::energy_per_round(p1, true);
                let e2 = Self::energy_per_round(p2, true);

                match e1.partial_cmp(&e2) {
                    Some(ord) if ord != Ordering::Equal => ord,
                    _ => {
                        // Tie-breaker: fall back to "smaller code" heuristic:
                        // fewer physical qubits, then shorter cycle time.
                        if let (Ok(num_qubits1), Ok(t_cycle1), Ok(num_qubits2), Ok(t_cycle2)) = (
                            self.physical_qubits(p1),
                            self.logical_cycle_time(qubit, p1),
                            self.physical_qubits(p2),
                            self.logical_cycle_time(qubit, p2),
                        ) {
                            num_qubits1.cmp(&num_qubits2).then(t_cycle1.cmp(&t_cycle2))
                        } else {
                            Ordering::Equal
                        }
                    }
                }
            }

            OptimizationTarget::Qubits => {
                // --- original qubit-optimized comparator ---
                if let (
                    Ok(num_qubits1),
                    Ok(logical_cycle_time1),
                    Ok(num_qubits2),
                    Ok(logical_cycle_time2),
                ) = (
                    self.physical_qubits(p1),
                    self.logical_cycle_time(qubit, p1),
                    self.physical_qubits(p2),
                    self.logical_cycle_time(qubit, p2),
                ) {
                    num_qubits1
                        .cmp(&num_qubits2)
                        .then(logical_cycle_time1.cmp(&logical_cycle_time2))
                } else {
                    Ordering::Equal
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64, relative_tolerance: f64) {
        assert!(
            (actual - expected).abs() <= relative_tolerance * expected.abs(),
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    /// Expected values from arXiv:2302.06639 (eq. 3, eq. 4, eq. D8):
    /// `d*(5.6e-2*(alpha_sq^0.86 * k1_k2/0.013)^((d+1)/2) + 2*(d-1)*0.5*exp(-2*alpha_sq))`.
    fn logical_error_rate_matches_paper_formula() {
        let qec = RepetitionCode::new();
        let qubit = CatQubit::new();

        let cases = [
            (3, 3.75, 3.3195e-3),
            (5, 7.15, 1.2341e-5),
            (7, 9.71, 1.5496e-7),
            (9, 11.64, 5.5894e-9),
            (11, 15.0, 1.0443e-11),
            (13, 20.0, 8.5464e-15),
        ];

        for (distance, alpha_sq, expected) in cases {
            let parameter = CodeParameter::new(distance, alpha_sq);
            let actual = qec
                .logical_error_rate(&qubit, &parameter)
                .expect("logical error rate should compute");
            assert_close(actual, expected, 1e-4);
        }
    }

    #[test]
    fn physical_qubits_is_two_d_minus_one() {
        let qec = RepetitionCode::new();
        for distance in [1, 3, 5, 9, 49] {
            let parameter = CodeParameter::new(distance, 10.0);
            assert_eq!(
                qec.physical_qubits(&parameter).expect("should compute"),
                2 * distance - 1
            );
        }
    }

    #[test]
    fn logical_cycle_time_is_500_times_d() {
        let qec = RepetitionCode::new();
        let qubit = CatQubit::new();
        for distance in [1, 3, 5, 9, 49] {
            let parameter = CodeParameter::new(distance, 10.0);
            assert_eq!(
                qec.logical_cycle_time(&qubit, &parameter)
                    .expect("should compute"),
                500 * distance
            );
        }
    }

    #[test]
    /// Checks the bit-flip-dominated regime only. Past `alpha_sq` ~17 (at
    /// d=9) the phase-flip term takes over and the rate rises again; that
    /// is expected, not a defect.
    fn logical_error_rate_decreases_with_alpha_sq() {
        let qec = RepetitionCode::new();
        let qubit = CatQubit::new();
        let distance = 9;

        let mut previous = f64::INFINITY;
        for alpha_sq in 3..=16 {
            let parameter = CodeParameter::new(distance, f64::from(alpha_sq));
            let current = qec
                .logical_error_rate(&qubit, &parameter)
                .expect("should compute");
            assert!(
                current < previous,
                "logical_error_rate should decrease as alpha_sq grows (alpha_sq={alpha_sq})"
            );
            previous = current;
        }
    }
}
