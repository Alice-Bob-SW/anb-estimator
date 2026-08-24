// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Convenience structure to display resource estimation results.

#[cfg(any(feature = "cli", feature = "python"))]
use std::fmt::Display;
use std::ops::Deref;

#[cfg(any(feature = "cli", feature = "python"))]
use num_traits::FromPrimitive;
use num_traits::ToPrimitive;
#[cfg(feature = "python")]
use pyo3::pyclass;
use resource_estimator::estimates::{
    ErrorBudget, FactoryPart, Overhead, PhysicalResourceEstimationResult,
};

use crate::{code::RepetitionCode, counter::LogicalCounts, factories::ToffoliFactory};

/// Represents a physical resources estimate for Alice & Bob's architecture.
pub struct AliceAndBobEstimates(
    PhysicalResourceEstimationResult<RepetitionCode, ToffoliFactory, LogicalCounts>,
);

impl AliceAndBobEstimates {
    #[must_use]
    /// Give a reference to the [`FactoryPart`] used in the estimate.
    pub fn toffoli_factory_part(&self) -> Option<&FactoryPart<ToffoliFactory>> {
        self.factory_parts()[0].as_ref()
    }

    #[must_use]
    /// Count the number of physical qubits, routing qubits included.
    pub fn physical_qubits(&self) -> u64 {
        // "Vertical" routing qubits must be added to ensure all-to-all connectivity
        // Formula from arXiv: 2302.06639, p. 27. `logical_qubits()` include the "horizontal
        // routing qubits", including the one between the computation qubits and factories.
        let additional_routing_qubits = 2
            * ((3
                * (self.layout_overhead().logical_qubits()
                    + self.toffoli_factory_part().map_or(0, FactoryPart::copies) * 5))
                - 1);
        self.0.physical_qubits() + additional_routing_qubits
    }

    #[must_use]
    /// Compute the percentage of physical qubits allocated to the Toffoli magic
    /// states factories.
    pub fn factory_fraction(&self) -> f64 {
        (self
            .physical_qubits_for_factories()
            .to_f64()
            .expect("can't convert")
            / self.physical_qubits().to_f64().expect("can't convert"))
            * 100.0
    }

    #[must_use]
    /// Compute the total error of the computation
    pub fn total_error(&self) -> f64 {
        // Error is computed as 'logical + magic' without the cross term since it is
        // largely sub-leading here, and negative anyway
        let logical = (self.num_cycles() * self.layout_overhead().logical_qubits())
            .to_f64()
            .expect("can't convert volume as f64")
            * self.logical_patch().logical_error_rate();
        let magic_states = self.toffoli_factory_part().map_or(0.0, |p| {
            self.num_magic_states(0)
                .to_f64()
                .expect("can't convert number of magic states as f64")
                * p.factory().error_probability()
        });

        logical + magic_states
    }
}

impl Deref for AliceAndBobEstimates {
    type Target = PhysicalResourceEstimationResult<RepetitionCode, ToffoliFactory, LogicalCounts>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<PhysicalResourceEstimationResult<RepetitionCode, ToffoliFactory, LogicalCounts>>
    for AliceAndBobEstimates
{
    fn from(
        value: PhysicalResourceEstimationResult<RepetitionCode, ToffoliFactory, LogicalCounts>,
    ) -> Self {
        Self(value)
    }
}

/// Plain snapshot of an [`AliceAndBobEstimates`], shared by the CLI's JSON/text
/// output and the Python bindings.
#[cfg(any(feature = "cli", feature = "python"))]
#[cfg_attr(feature = "python", pyclass(frozen, get_all))]
#[cfg_attr(feature = "cli", derive(serde::Serialize))]
#[derive(Clone)]
pub struct EstimatesReport {
    /// Number of physical qubits, routing qubits included.
    pub physical_qubits: u64,
    /// Runtime, in seconds.
    pub runtime_seconds: f64,
    /// Runtime, in hours.
    pub runtime_hours: f64,
    /// Total error probability of the computation.
    pub total_error: f64,

    /// Code distance of the logical patch.
    pub code_distance: u64,
    /// Average number of photons |α|² in each cat qubit of the logical patch.
    pub code_alpha2: f64,

    /// Number of Toffoli factory copies.
    pub factories: u64,
    /// Code distance used inside the Toffoli factories.
    pub factories_distance: u64,
    /// Average number of photons |α|² in each cat qubit used in factories.
    pub factories_alpha2: f64,

    /// Fraction of physical qubits allocated to the Toffoli factories, in percent.
    pub factory_fraction_percent: f64,
    /// Fraction of physical qubits allocated to the Toffoli factories, in \[0, 1\].
    pub factory_fraction: f64,
}

#[cfg(any(feature = "cli", feature = "python"))]
impl From<&AliceAndBobEstimates> for EstimatesReport {
    fn from(e: &AliceAndBobEstimates) -> Self {
        let code_parameter = e.logical_patch().code_parameter();
        let factory_part = e.toffoli_factory_part().expect("No factory part");
        let factory = factory_part.factory();

        Self {
            physical_qubits: e.physical_qubits(),
            runtime_seconds: f64::from_u64(e.runtime()).expect("runtime is too large") / 1e9,
            runtime_hours: f64::from_u64(e.runtime()).expect("runtime is too large") / 1e9 / 3600.0,
            total_error: e.total_error(),
            code_distance: code_parameter.distance,
            code_alpha2: code_parameter.alpha_sq,
            factories: factory_part.copies(),
            factories_distance: factory.code_parameter.distance,
            factories_alpha2: factory.code_parameter.alpha_sq,
            factory_fraction_percent: e.factory_fraction(),
            factory_fraction: e.factory_fraction() / 100.0,
        }
    }
}

#[cfg(any(feature = "cli", feature = "python"))]
impl Display for EstimatesReport {
    /// Print the final estimates.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f)?;
        writeln!(f, "─────────────────────────────")?;
        writeln!(f, "# physical qubits:    {}", self.physical_qubits)?;
        writeln!(f, "runtime:             {:.2} hrs", self.runtime_hours)?;
        writeln!(f, "total error:         {:.5}", self.total_error)?;
        writeln!(f, "─────────────────────────────")?;
        writeln!(
            f,
            "code distance:       {} (|ɑ|² = {})",
            self.code_distance, self.code_alpha2
        )?;
        writeln!(f, "#factories:          {}", self.factories)?;
        writeln!(
            f,
            "factories distance:  {} (|ɑ|² = {})",
            self.factories_distance, self.factories_alpha2
        )?;
        writeln!(
            f,
            "factory fraction:    {:.2}%",
            self.factory_fraction_percent
        )?;
        writeln!(f, "─────────────────────────────")
    }
}

/// Builds an [`ErrorBudget`] from either a total error target or a per-component budget.
///
/// # Arguments
/// - `error_total` — If `Some(p)`, split the total error `p` into equal
///   topological and magic error components `(0.5p, 0.5p)` with rotations error set to `0.0`.
/// - `error_budget` — If `Some((logical_error, magic_state_error, rotation_error))`, use these
///   explicit per-component values.
///
/// # Returns
/// An [`ErrorBudget`] containing the Proba of >= 1 logical error, the proba of >= 1 faulty magic state distillation,
///     and the proba of >= 1 failed rotation synthesis), or an error.
///
/// # Notes
/// - If both `error_total` and `error_budget` are `None`, a default split of
///   `(0.333*0.5, 0.333*0.5, 0.0)` is used (conservative placeholder).
/// - Supplying both `Some` variants returns an error.
pub fn make_budget(
    error_total: Option<f64>,
    error_budget: Option<(f64, f64, f64)>,
) -> Result<ErrorBudget, &'static str> {
    match (error_total, error_budget) {
        (Some(p), None) => Ok(ErrorBudget::new(p * 0.5, p * 0.5, 0.0)),
        (None, Some((logical_error, magic_state_error, rotation_error))) => Ok(ErrorBudget::new(
            logical_error,
            magic_state_error,
            rotation_error,
        )),
        (None, None) => Ok(ErrorBudget::new(0.333 * 0.5, 0.333 * 0.5, 0.0)),
        (Some(_), Some(_)) => Err("Provide either error_total or error_budget, not both."),
    }
}
