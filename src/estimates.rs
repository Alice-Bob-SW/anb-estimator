//! Convenience structure to display resource estimation results.

#[cfg(any(feature = "cli", feature = "python"))]
use std::fmt::Display;
use std::ops::Deref;

use std::rc::Rc;

#[cfg(any(feature = "cli", feature = "python"))]
use num_traits::FromPrimitive;
use num_traits::ToPrimitive;
#[cfg(feature = "python")]
use pyo3::pyclass;
use resource_estimator::estimates::{
    self, ErrorBudget, ErrorCorrection, Factory, FactoryPart, LogicalPatch, Overhead,
    PhysicalResourceEstimation, PhysicalResourceEstimationResult, RealizedOverhead,
};

use crate::{
    code::{CodeParameter, RepetitionCode},
    counter::LogicalCounts,
    factories::{ToffoliBuilder, ToffoliFactory},
    qubit::CatQubit,
};

/// Represents a physical resources estimate for Alice & Bob's architecture.
pub struct AliceAndBobEstimates(PhysicalResourceEstimationResult<RepetitionCode, ToffoliFactory>);

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
        //
        // The boundary patch between the factories and the main compute block must be
        // wide enough for both sides to stay all-to-all connected, so it is sized to the
        // larger of the two code distances. `boundary_correction` adds the qubits needed
        // to widen the interface from the mismatch between the two distances.
        let main_distance = self.logical_patch().code_parameter().distance;
        let (copies, boundary_correction) = self.toffoli_factory_part().map_or((0, 0), |part| {
            let factory_distance = part.factory().code_parameter.distance;
            (part.copies(), 2 * factory_distance.abs_diff(main_distance))
        });
        let additional_routing_qubits =
            2 * ((3 * (self.layout_overhead().logical_qubits() + copies * 5)) - 1);

        self.0.physical_qubits() + additional_routing_qubits + boundary_correction
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

    /// Builds an estimate from a fixed code parameter, factory, and factory
    /// count.
    ///
    /// `num_factories` must be at least 1. The factory's own code parameter
    /// is independent of `code_parameter`; look it up with
    /// [`ToffoliBuilder::factories`](crate::ToffoliBuilder::factories).
    pub fn from_fixed_parameters(
        count: LogicalCounts,
        code_parameter: CodeParameter,
        factory: ToffoliFactory,
        num_factories: u64,
    ) -> Result<Self, estimates::Error> {
        let qubit = Rc::new(CatQubit::new());
        let qec = RepetitionCode::new();
        let layout = Rc::new(count);

        let logical_patch = LogicalPatch::new(&qec, code_parameter, qubit.clone())?;

        // `LogicalCounts`'s `Overhead` impl ignores the budget.
        let budget = ErrorBudget::new(0.0, 0.0, 0.0);
        let realized_overhead = RealizedOverhead::from_overhead(&*layout, &budget, 1)?;
        let num_magic_states = realized_overhead.num_magic_states()[0];

        // Number of logical cycles: the larger of the cycles the algorithm
        // needs and the cycles needed for magic-state production at this
        // factory count.
        let required_runs = num_magic_states.div_ceil(num_factories * factory.num_output_states());
        let required_duration = required_runs * factory.duration();
        let num_cycles_for_magic_states =
            required_duration.div_ceil(logical_patch.logical_cycle_time());
        let num_cycles = realized_overhead
            .logical_depth()
            .max(num_cycles_for_magic_states);

        let factory_error_probability = factory.error_probability();
        let factory_part = FactoryPart::new(
            factory,
            num_factories,
            num_magic_states,
            factory_error_probability,
        );

        let estimation =
            PhysicalResourceEstimation::new(qec, qubit, ToffoliBuilder::default(), layout);
        Ok(PhysicalResourceEstimationResult::new(
            &estimation,
            logical_patch,
            &budget,
            num_cycles,
            vec![Some(factory_part)],
            0.0, // required_logical_error_rate: unused by `EstimatesReport`/`total_error`
        )?
        .into())
    }
}

/// Error returned by [`estimate_resources`] and [`estimate_frontier`].
#[derive(Debug)]
pub enum EstimationError {
    /// The underlying resource estimator failed.
    Estimator(estimates::Error),
    /// No estimate fits within the physical qubit cap.
    MaxPhysicalQubitsTooSmall {
        /// Requested cap.
        max_physical_qubits: u64,
        /// Physical qubit count of the smallest estimate found, if any.
        min_physical_qubits: Option<u64>,
    },
}

impl std::fmt::Display for EstimationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Estimator(e) => write!(f, "{e}"),
            Self::MaxPhysicalQubitsTooSmall {
                max_physical_qubits,
                min_physical_qubits: Some(min),
            } => write!(
                f,
                "max physical qubits {max_physical_qubits} is too small: \
                 the smallest estimate needs {min} physical qubits"
            ),
            Self::MaxPhysicalQubitsTooSmall {
                max_physical_qubits,
                min_physical_qubits: None,
            } => write!(
                f,
                "max physical qubits {max_physical_qubits} is too small: no estimate found"
            ),
        }
    }
}

impl std::error::Error for EstimationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Estimator(e) => Some(e),
            Self::MaxPhysicalQubitsTooSmall { .. } => None,
        }
    }
}

impl From<estimates::Error> for EstimationError {
    fn from(e: estimates::Error) -> Self {
        Self::Estimator(e)
    }
}

fn new_estimation(
    count: LogicalCounts,
) -> PhysicalResourceEstimation<RepetitionCode, ToffoliBuilder, LogicalCounts> {
    PhysicalResourceEstimation::new(
        RepetitionCode::new(),
        Rc::new(CatQubit::new()),
        ToffoliBuilder::default(),
        Rc::new(count),
    )
}

/// Estimates the physical resources for `count` within `budget`.
///
/// Without `max_physical_qubits`, returns the estimate chosen by the
/// underlying resource estimator. With `max_physical_qubits`, returns the
/// fastest estimate of [`estimate_frontier`] under the same cap. This
/// estimate can use more qubits than the one without a cap, even when the cap
/// is larger than both.
///
/// # Errors
/// Same as [`estimate_frontier`].
pub fn estimate_resources(
    count: LogicalCounts,
    budget: &ErrorBudget,
    max_physical_qubits: Option<u64>,
) -> Result<AliceAndBobEstimates, EstimationError> {
    if max_physical_qubits.is_none() {
        return Ok(new_estimation(count).estimate(budget)?.into());
    }
    let mut frontier = estimate_frontier(count, budget, max_physical_qubits)?;
    let fastest = fastest_index(&frontier).expect("frontier is not empty");
    Ok(frontier.swap_remove(fastest))
}

/// Builds the Pareto frontier of estimates for `count` within `budget`,
/// sorted by decreasing number of physical qubits.
///
/// Without `max_physical_qubits`, returns the frontier of the underlying
/// resource estimator. That frontier is Pareto-optimal for the qubit count of
/// the underlying estimator, which excludes the routing qubits added by
/// [`AliceAndBobEstimates::physical_qubits`].
///
/// With `max_physical_qubits`, searches all code parameters, factories and
/// factory counts, and keeps the Pareto-optimal estimates whose
/// [`AliceAndBobEstimates::physical_qubits`] is at most the cap.
///
/// # Errors
/// - [`EstimationError::Estimator`] if the underlying estimator fails.
/// - [`EstimationError::MaxPhysicalQubitsTooSmall`] if no estimate fits
///   within `max_physical_qubits`.
pub fn estimate_frontier(
    count: LogicalCounts,
    budget: &ErrorBudget,
    max_physical_qubits: Option<u64>,
) -> Result<Vec<AliceAndBobEstimates>, EstimationError> {
    let Some(max_physical_qubits) = max_physical_qubits else {
        return Ok(new_estimation(count)
            .build_frontier(budget)?
            .into_iter()
            .map(Into::into)
            .collect());
    };

    let frontier = search_frontier(&count, budget, max_physical_qubits)?;
    if frontier.is_empty() {
        let min_physical_qubits = search_frontier(&count, budget, u64::MAX)?
            .iter()
            .map(AliceAndBobEstimates::physical_qubits)
            .min();
        return Err(EstimationError::MaxPhysicalQubitsTooSmall {
            max_physical_qubits,
            min_physical_qubits,
        });
    }
    Ok(frontier)
}

/// Searches all code parameters, factories and factory counts, and returns
/// the Pareto-optimal estimates with at most `max_physical_qubits` qubits,
/// sorted by decreasing number of physical qubits.
///
/// An estimate is valid if it meets both the logical error budget and the
/// magic state error budget. Among estimates with the same qubit count and
/// runtime, the first one in search order is kept, that is the one with the
/// smallest code parameter and the smallest factory index.
fn search_frontier(
    count: &LogicalCounts,
    budget: &ErrorBudget,
    max_physical_qubits: u64,
) -> Result<Vec<AliceAndBobEstimates>, estimates::Error> {
    let num_magic_states = count
        .num_magic_states(budget, 0)
        .map_err(estimates::Error::NumberOfMagicStatesComputationFailed)?;
    if num_magic_states == 0 {
        return Ok(new_estimation(count.clone())
            .build_frontier(budget)?
            .into_iter()
            .map(AliceAndBobEstimates::from)
            .filter(|e| e.physical_qubits() <= max_physical_qubits)
            .collect());
    }
    let logical_qubits = count
        .logical_qubits()
        .map_err(estimates::Error::AlgorithmicLogicalQubitsComputationFailed)?
        .to_f64()
        .expect("can't convert logical qubits as f64");
    let min_cycles = count
        .logical_depth(budget)
        .map_err(estimates::Error::AlgorithmicLogicalDepthComputationFailed)?;
    let magic_states_f64 = num_magic_states
        .to_f64()
        .expect("can't convert number of magic states as f64");

    let qec = RepetitionCode::new();
    let qubit = Rc::new(CatQubit::new());
    let builder = ToffoliBuilder::default();

    // (physical qubits, runtime, code parameter, factory index, factory count)
    let mut points = Vec::new();
    for code_parameter in qec.code_parameter_range(None) {
        let patch = LogicalPatch::new(&qec, code_parameter.clone(), qubit.clone())?;
        let logical_error_per_cycle = logical_qubits * patch.logical_error_rate();
        let cycles_error =
            |cycles: u64| cycles.to_f64().expect("can't convert cycles") * logical_error_per_cycle;
        if cycles_error(min_cycles) > budget.logical() {
            continue;
        }
        for (factory_index, factory) in builder.factories().iter().enumerate() {
            if magic_states_f64 * factory.error_probability() > budget.magic_states() {
                continue;
            }
            // More factories add qubits and never add cycles. Only factory
            // counts that reduce the number of factory runs can be faster.
            let states_per_factory = factory.num_output_states();
            let mut num_factories = 1;
            loop {
                let estimate = AliceAndBobEstimates::from_fixed_parameters(
                    count.clone(),
                    code_parameter.clone(),
                    factory.clone(),
                    num_factories,
                )?;
                if estimate.physical_qubits() > max_physical_qubits {
                    break;
                }
                if cycles_error(estimate.num_cycles()) <= budget.logical() {
                    points.push((
                        estimate.physical_qubits(),
                        estimate.runtime(),
                        code_parameter.clone(),
                        factory_index,
                        num_factories,
                    ));
                }
                let runs = num_magic_states.div_ceil(num_factories * states_per_factory);
                if runs <= 1 || estimate.num_cycles() <= min_cycles {
                    break;
                }
                // Smallest factory count with at most `runs - 1` runs.
                num_factories = num_magic_states.div_ceil((runs - 1) * states_per_factory);
            }
        }
    }

    // Stable sort, so ties keep the search order.
    points.sort_by_key(|&(qubits, runtime, ..)| (qubits, runtime));
    let mut frontier = Vec::new();
    let mut best_runtime = u64::MAX;
    for (_, runtime, code_parameter, factory_index, num_factories) in points {
        if runtime < best_runtime {
            best_runtime = runtime;
            frontier.push(AliceAndBobEstimates::from_fixed_parameters(
                count.clone(),
                code_parameter,
                builder.factories()[factory_index].clone(),
                num_factories,
            )?);
        }
    }
    frontier.reverse();
    Ok(frontier)
}

/// Returns the index of the fastest estimate. Ties go to the estimate with
/// fewer physical qubits.
pub(crate) fn fastest_index(estimates: &[AliceAndBobEstimates]) -> Option<usize> {
    estimates
        .iter()
        .enumerate()
        .min_by_key(|(_, e)| (e.runtime(), e.physical_qubits()))
        .map(|(i, _)| i)
}

impl Deref for AliceAndBobEstimates {
    type Target = PhysicalResourceEstimationResult<RepetitionCode, ToffoliFactory>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<PhysicalResourceEstimationResult<RepetitionCode, ToffoliFactory>>
    for AliceAndBobEstimates
{
    fn from(value: PhysicalResourceEstimationResult<RepetitionCode, ToffoliFactory>) -> Self {
        Self(value)
    }
}

/// Plain snapshot of an [`AliceAndBobEstimates`], shared by the CLI's JSON/text
/// output and the Python bindings.
#[cfg(any(feature = "cli", feature = "python"))]
#[cfg_attr(feature = "python", pyclass(frozen, get_all, skip_from_py_object, str))]
#[cfg_attr(any(feature = "cli", feature = "python"), derive(serde::Serialize))]
#[derive(Clone, Debug)]
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
