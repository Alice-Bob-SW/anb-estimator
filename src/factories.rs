// Copyright (c) Microsoft Corporation.
// Copyright (c) Alice & Bob.
// Licensed under the MIT License.

//! Toffoli magic states factories.
//!
//! The factories are based on fault-tolerant measurement of stabilizers of the
//! Toffoli magic state, as described in
//! [arXiv:2302.06639](https://arxiv.org/abs/2302.06639).
//!
//! In the article, the performances for some parameter sets have been
//! precomputed (see Table III, p. 35). The table is hard-coded in the
//! implementation of [`Default`] for [`ToffoliBuilder`].
//! Note that 1/κ₂ = 100 ns and κ₁/κ₂ = 1e-5 are hard-coded (values used in the
//! precomputation).

use num_traits::FromPrimitive;
use resource_estimator::estimates::{self, FactoryBuilder};
use std::{borrow::Cow, fmt::Display, rc::Rc};

use crate::{code::CodeParameter, CatQubit, RepetitionCode};

/// Struct containing parameters of Toffoli magic states factories based on
/// fault-tolerant measurement of stabilizers of the Toffoli magic state.
///
/// A factory is described by:
/// - the internal code distance (usually differs from the one of the main part
///   of the processor)
/// - average photon number |α|² inside the factory (also separate from the one
///   used in the rest of the processor)
/// - logical error probability
/// - acceptance probability (they are heralded)
/// - the number of steps it uses
///
/// Note that this code does not compute the performance (error and acceptance
/// probabilities, time, etc.) of the factories, but uses them. Hence it must be
/// precomputed outside of the resource estimator (that will only choose one of
/// the proposed factories).
///
/// Performance of Toffoli magic states factories for different sets of
/// parameters have been precomputed in
/// [arXiv:2302.06639](https://arxiv.org/abs/2302.06639) (Table III, p. 35) and
/// are available through [`ToffoliBuilder`]'s [`Default`] trait.
///
/// Value 1/κ₂ = 100 ns and κ₁/κ₂ = 1e-5 are hard-coded.
#[derive(Clone, PartialEq)]
pub struct ToffoliFactory {
    pub(crate) code_parameter: CodeParameter,
    error_probability: f64,
    acceptance_probability: f64,
    steps: usize,
}

impl ToffoliFactory {
    /// Logical error probability of the magic state preparation.
    #[must_use]
    pub fn error_probability(&self) -> f64 {
        self.error_probability
    }

    /// Space-time volume of the factory (including retries).
    #[must_use]
    pub fn normalized_volume(&self) -> u64 {
        // Could have been derived from Factory, but different return type.
        use estimates::Factory;

        assert_eq!(self.num_output_states(), 1);

        self.physical_qubits() * self.duration()
    }
}

impl estimates::Factory for ToffoliFactory {
    type Parameter = CodeParameter;

    /// Number of physical qubits in each factory.
    ///
    /// Each Toffoli factory requires 4 logical qubits + 1 "horizontal" routing qubit,
    /// see [arXiv:2302.06639](https://arxiv.org/abs/2302.06639) (p. 27).
    /// The routing qubit under the factories is associated with the compute qubits.
    ///
    /// Note that the formula might not be exact when factories internal distance is
    /// different than the main code distance, but it is negligible.
    /// Additionally, note that this might not even be a real problem as only one of the 4
    /// factory qubits needs to be accessed through all its physical qubits.
    fn physical_qubits(&self) -> u64 {
        let num_logical_qubits: u64 = 4;
        let horizontal_routing_qubits: u64 = 1;

        (num_logical_qubits + horizontal_routing_qubits) * (2 * self.code_parameter.distance - 1)
    }

    /// Average duration of the magic state preparation.
    ///
    /// Note that contrarily to the code used in the main part of the processor,
    /// as in [arXiv:2302.06639](https://arxiv.org/abs/2302.06639) the CNOTs are
    /// implemented in an adiabatic way, with a gate time of 89.2/(κ₂|α|²) (see
    /// p. 32).
    /// 1/κ₂ = 100 ns and κ₁/κ₂ = 1e-5 are hard-coded.
    ///
    /// The factory is heralded, this duration take into account that retry
    /// might be required.
    fn duration(&self) -> u64 {
        // If you change it, you also need to recompute the default factories.
        let t = 100.0; // 1/κ₂ [nanoseconds]

        // The more accurate # of time steps 89.2 was taken from the Github code
        // (vs 89 in arXiv:2302.06639 (p. 32))
        // Complete formula is: π/(8 |α|^2 sqrt(2κ₁κ₂)). Using it would allow to
        // change κ₂ at κ₁/κ₂ constant.
        let gate_time = 89.2 * t / self.code_parameter.alpha_sq;

        f64::from_usize(self.steps)
            .map(|steps| (gate_time * steps / self.acceptance_probability).round())
            .and_then(u64::from_f64)
            .expect("Cannot compute runtime of factory.")
    }

    fn num_output_states(&self) -> u64 {
        1
    }

    fn max_code_parameter(&self) -> Option<Cow<'_, Self::Parameter>> {
        Some(Cow::Owned(CodeParameter::new(
            self.code_parameter.distance,
            self.code_parameter.alpha_sq,
        )))
    }
}

impl Eq for ToffoliFactory {}

impl Ord for ToffoliFactory {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.normalized_volume().cmp(&other.normalized_volume())
    }
}

impl PartialOrd for ToffoliFactory {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Display for ToffoliFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (|ɑ|² = {})",
            self.code_parameter.distance, self.code_parameter.alpha_sq
        )
    }
}

/// Contains a bunch of factories, and knows how to choose the best one.
pub struct ToffoliBuilder {
    factories: Vec<ToffoliFactory>,
    lowest_error_probability: f64,
}

impl Default for ToffoliBuilder {
    #[allow(clippy::too_many_lines)]
    /// Factories from [arXiv:2302.06639](https://arxiv.org/abs/2302.06639),
    /// p.35, Table III.
    fn default() -> Self {
        let factories = vec![
            ToffoliFactory {
                code_parameter: CodeParameter::new(3, 3.75),
                error_probability: 1.05e-3,
                steps: 23,
                acceptance_probability: 0.84,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(3, 5.08),
                error_probability: 1.02e-4,
                steps: 29,
                acceptance_probability: 0.745,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(3, 5.32),
                error_probability: 8.14e-5,
                steps: 35,
                acceptance_probability: 0.66,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(5, 7.15),
                error_probability: 4.62e-6,
                steps: 46,
                acceptance_probability: 0.456,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(5, 8.18),
                error_probability: 7.00e-7,
                steps: 53,
                acceptance_probability: 0.362,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(5, 8.38),
                error_probability: 5.36e-7,
                steps: 60,
                acceptance_probability: 0.288,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(7, 9.71),
                error_probability: 6.14e-8,
                steps: 73,
                acceptance_probability: 0.148,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(7, 10.76),
                error_probability: 8.40e-9,
                steps: 81,
                acceptance_probability: 0.105,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(7, 11.06),
                error_probability: 5.16e-9,
                steps: 89,
                acceptance_probability: 0.0727,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(9, 11.64),
                error_probability: 2.28e-9,
                steps: 104,
                acceptance_probability: 0.0262,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(9, 12.83),
                error_probability: 2.30e-10,
                steps: 113,
                acceptance_probability: 0.0154,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(9, 13.44),
                error_probability: 7.36e-11,
                steps: 122,
                acceptance_probability: 0.00975,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(19, 17.35),
                error_probability: 7.90e-12,
                steps: 9576,
                acceptance_probability: 1.0,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(21, 18.94),
                error_probability: 5.40e-13,
                steps: 14112,
                acceptance_probability: 1.0,
            },
            ToffoliFactory {
                code_parameter: CodeParameter::new(23, 20.53),
                error_probability: 3.74e-14,
                steps: 21344,
                acceptance_probability: 1.0,
            },
        ];

        let lowest_error_probability = factories
            .iter()
            .map(|f| f.error_probability)
            .min_by(f64::total_cmp)
            .unwrap_or_default();

        Self {
            factories,
            lowest_error_probability,
        }
    }
}

impl ToffoliBuilder {
    /// The factories in this builder, in Table III order.
    #[must_use]
    pub fn factories(&self) -> &[ToffoliFactory] {
        &self.factories
    }
}

impl FactoryBuilder<RepetitionCode> for ToffoliBuilder {
    type Factory = ToffoliFactory;

    /// Provide a sorted (by volume) list of factories that reach the target
    /// logical error rate.
    fn find_factories(
        &self,
        _ftp: &RepetitionCode,
        _qubit: &Rc<CatQubit>,
        _magic_state_type: usize,
        output_error_rate: f64,
        _max_code_parameter: &CodeParameter,
    ) -> Option<Vec<Cow<'_, Self::Factory>>> {
        assert!(
            output_error_rate > self.lowest_error_probability,
            "Requested error probability is too low"
        );

        let mut factories: Vec<_> = self
            .factories
            .iter()
            .filter_map(|factory| {
                (factory.error_probability <= output_error_rate).then_some(Cow::Borrowed(factory))
            })
            .collect();
        factories.sort_unstable();
        Some(factories)
    }

    /// Number of types of magic states.
    fn num_magic_state_types(&self) -> usize {
        // Same implementation as the provided one.
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use estimates::Factory;
    use std::rc::Rc;

    /// Transcribed directly from arXiv:2302.06639 Table III (p. 35).
    const TABLE_III: [(u64, f64, f64, usize, f64); 15] = [
        (3, 3.75, 1.05e-3, 23, 0.84),
        (3, 5.08, 1.02e-4, 29, 0.745), // alpha_sq is 3.93 in v2 of the paper, should be 5.08 (typo)
        (3, 5.32, 8.14e-5, 35, 0.66),
        (5, 7.15, 4.62e-6, 46, 0.456),
        (5, 8.18, 7.00e-7, 53, 0.362),
        (5, 8.38, 5.36e-7, 60, 0.288),
        (7, 9.71, 6.14e-8, 73, 0.148),
        (7, 10.76, 8.40e-9, 81, 0.105),
        (7, 11.06, 5.16e-9, 89, 0.0727),
        (9, 11.64, 2.28e-9, 104, 0.0262),
        (9, 12.83, 2.30e-10, 113, 0.0154),
        (9, 13.44, 7.36e-11, 122, 0.00975),
        (19, 17.35, 7.90e-12, 9576, 1.0),
        (21, 18.94, 5.40e-13, 14112, 1.0),
        (23, 20.53, 3.74e-14, 21344, 1.0),
    ];

    #[test]
    fn default_factories_match_table_iii() {
        let builder = ToffoliBuilder::default();
        assert_eq!(builder.factories.len(), TABLE_III.len());

        for (factory, (distance, alpha_sq, error_probability, steps, acceptance_probability)) in
            builder.factories.iter().zip(TABLE_III)
        {
            assert_eq!(factory.code_parameter.distance, distance);
            assert!((factory.code_parameter.alpha_sq - alpha_sq).abs() < 1e-9);
            assert!(
                (factory.error_probability() - error_probability).abs() <= error_probability * 1e-9
            );
            assert_eq!(factory.steps, steps);
            assert!((factory.acceptance_probability - acceptance_probability).abs() < 1e-9);
        }
    }

    #[test]
    /// A factory has 5 lines: 4 logical qubits and 1 routing qubit. Each
    /// line has 2d-1 physical qubits: d data qubits and d-1 ancilla qubits
    /// (arXiv:2302.06639, Fig. 25).
    fn physical_qubits_is_five_times_two_d_minus_one() {
        for factory in ToffoliBuilder::default().factories {
            let d = factory.code_parameter.distance;
            assert_eq!(factory.physical_qubits(), 5 * (2 * d - 1));
        }
    }

    #[test]
    fn duration_matches_formula() {
        for factory in ToffoliBuilder::default().factories {
            let t = 100.0; // 1/κ₂ [nanoseconds]
            let gate_time = 89.2 * t / factory.code_parameter.alpha_sq;
            let steps = f64::from_usize(factory.steps).expect("steps should convert to f64");
            let expected =
                u64::from_f64((gate_time * steps / factory.acceptance_probability).round())
                    .expect("duration should convert to u64");
            assert_eq!(factory.duration(), expected);
        }
    }

    #[test]
    fn find_factories_filters_by_error_and_sorts_by_volume() {
        let builder = ToffoliBuilder::default();
        let qec = RepetitionCode::new();
        let qubit = Rc::new(CatQubit::new());
        let output_error_rate = 1e-6;

        let factories = builder
            .find_factories(
                &qec,
                &qubit,
                0,
                output_error_rate,
                &CodeParameter::new(49, 30.0),
            )
            .expect("should find factories");

        assert!(!factories.is_empty());
        for factory in &factories {
            assert!(factory.error_probability() <= output_error_rate);
        }
        for pair in factories.windows(2) {
            assert!(pair[0].normalized_volume() <= pair[1].normalized_volume());
        }
    }

    #[test]
    #[should_panic(expected = "Requested error probability is too low")]
    fn find_factories_panics_below_lowest_error_probability() {
        let builder = ToffoliBuilder::default();
        let qec = RepetitionCode::new();
        let qubit = Rc::new(CatQubit::new());

        let _ = builder.find_factories(&qec, &qubit, 0, 3.74e-14, &CodeParameter::new(49, 30.0));
    }
}
