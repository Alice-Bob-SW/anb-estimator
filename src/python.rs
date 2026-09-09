//! Python bindings for the Alice & Bob Q# resource estimator.
//! ! This module exposes functions to estimate quantum resources from Q# programs
//! ! and explicit logical counts, returning structured results suitable for Python consumers.
//! ! It leverages PyO3 to create Python-callable functions and classes.

use num_traits::FromPrimitive;
use pyo3::prelude::*; // brings Python, PyResult, PyModule, Bound, etc.
use std::rc::Rc;

use crate::estimates::make_budget;
use crate::{
    AliceAndBobEstimates, CatQubit, EstimatesReport, LogicalCounts as InternalLogicalCounts,
    RepetitionCode, ToffoliBuilder,
};
use resource_estimator::estimates::PhysicalResourceEstimation;

/// Python-visible logical counts, either extracted from a Q# program or built
/// directly by a Python caller.
///
/// Exposes a minimal, read-only view sufficient for downstream analysis in Python.
/// Fields correspond to logical resources observed by the interpreter.
#[pyclass(frozen, skip_from_py_object)]
#[derive(Clone, Debug, serde::Serialize)]
#[allow(clippy::struct_field_names)]
pub struct LogicalCounts {
    /// Number of (algorithm) logical qubits allocated by the interpreter.
    #[pyo3(get)]
    qubit_count: u64,
    /// Number of logical CX-equivalent two-qubit gates (including CY/CZ and SWAP→3×CX).
    #[pyo3(get)]
    cx_count: u64,
    /// Number of logical CCX (Toffoli) gates.
    #[pyo3(get)]
    ccx_count: u64,
}

#[pymethods]
impl LogicalCounts {
    /// Builds a [`LogicalCounts`] from Python, accepting floats that represent
    /// integers (e.g. `3.0`) for convenience.
    ///
    /// # Errors
    /// A `ValueError` if any count is negative or is not integer-valued.
    #[allow(clippy::similar_names)]
    #[new]
    fn new(qubit_count: f64, cx_count: f64, ccx_count: f64) -> PyResult<Self> {
        #[allow(clippy::float_cmp)]
        fn to_uint(name: &str, val: f64) -> PyResult<u64> {
            if val < 0.0 {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "{name} must be >= 0"
                )));
            }
            if val != val.floor() {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "{name} must be an integer or a float representing an integer (e.g., 3.0)"
                )));
            }
            u64::from_f64(val).ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(format!("{name} is too large"))
            })
        }

        Ok(Self {
            qubit_count: to_uint("qubit_count", qubit_count)?,
            cx_count: to_uint("cx_count", cx_count)?,
            ccx_count: to_uint("ccx_count", ccx_count)?,
        })
    }

    fn __repr__(&self) -> String {
        format!("{self:?}")
    }
}

/// Converts an internal [`InternalLogicalCounts`] reference into a Python-visible [`LogicalCounts`].
///
/// Copies only primitive fields; no heap sharing is required.
impl From<&InternalLogicalCounts> for LogicalCounts {
    fn from(c: &InternalLogicalCounts) -> Self {
        Self {
            qubit_count: c.qubit_count,
            cx_count: c.cx_count,
            ccx_count: c.ccx_count,
        }
    }
}

/// Bundles a single estimate with its optional frontier and the logical
/// counts it was computed from: the full result of one estimation call.
#[pyclass(frozen, get_all, skip_from_py_object)]
#[derive(Clone, Debug, serde::Serialize)]
pub struct FullResults {
    estimates: EstimatesReport,
    frontier: Option<Vec<EstimatesReport>>,
    counts: LogicalCounts,
}

#[pymethods]
impl FullResults {
    fn __repr__(&self) -> String {
        format!("{self:?}")
    }

    /// Serializes to a JSON object string.
    ///
    /// # Errors
    /// Propagates any (unexpected) serialization failure as a `ValueError`.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(self)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }
}

/// Estimate resources from a Q# file and return both the best estimate and, optionally,
/// a frontier of Pareto-optimal trade-offs, together with the parsed logical counts.
///
/// # Arguments
/// - `filename` — Path to a Q# source file to be parsed and interpreted for counts.
/// - `frontier` — If `true`, also compute a frontier of estimates (e.g., different distances/α).
/// - `error_total` — argument of make_budget ; mutually exclusive with `error_budget`.
/// - `error_budget` — argument of make_budget ; mutually exclusive with `error_total`.
///
/// # Errors
/// - I/O or parsing failures when loading the Q# file,
/// - Failures during resource estimation.
///
#[pyfunction]
fn _estimate_qsharp_file(
    filename: &str,
    frontier: bool,
    error_total: Option<f64>,
    error_budget: Option<(f64, f64, f64)>,
) -> PyResult<FullResults> {
    // Build the estimation
    let qubit = CatQubit::new();
    let qec = RepetitionCode::new();
    let builder = ToffoliBuilder::default();
    let budget = make_budget(error_total, error_budget)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    // Put counts behind an Rc so we can both pass it into PRE and also derive a Python view
    let counts = Rc::new(
        InternalLogicalCounts::from_qsharp(filename)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.clone()))?,
    );
    let counts_py = LogicalCounts::from(counts.as_ref());

    let estimation = PhysicalResourceEstimation::new(
        qec,
        Rc::new(qubit),
        builder,
        counts.clone(), // share with PRE
    );

    // Single best estimate
    let single_est: AliceAndBobEstimates = estimation
        .estimate(&budget)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
        .into();
    let single_report = EstimatesReport::from(&single_est);

    // Optional frontier
    let frontier_report = if frontier {
        let results = estimation
            .build_frontier(&budget)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Some(
            results
                .into_iter()
                .map(|r| {
                    let est: AliceAndBobEstimates = r.into();
                    EstimatesReport::from(&est)
                })
                .collect(),
        )
    } else {
        None
    };

    Ok(FullResults {
        estimates: single_report,
        frontier: frontier_report,
        counts: counts_py,
    })
}

/// Estimate resources from explicit logical counts and return typed results,
/// optionally including a frontier of trade-offs.
///
/// # Arguments
/// - `qubits` — Logical (algorithm) qubit count.
/// - `cx` — Logical CX-equivalent two-qubit gate count.
/// - `ccx` — Logical CCX (Toffoli) gate count.
/// - `frontier` — If `true`, compute and return the frontier as structured objects.
/// - `error_total` — argument of make_budget ; mutually exclusive with `error_budget`.
/// - `error_budget` — argument of make_budget ; mutually exclusive with `error_total`.
///
/// # Errors
/// Propagates errors from the physical resource estimator.
#[pyfunction]
fn _estimate_logical_counts(
    // TODO: remove duplication between here and main.rs.
    qubits: u64,
    cx: u64,
    ccx: u64,
    frontier: bool,
    error_total: Option<f64>,
    error_budget: Option<(f64, f64, f64)>,
) -> PyResult<FullResults> {
    // Build the estimation
    let qubit = CatQubit::new();
    let qec = RepetitionCode::new();
    let builder = ToffoliBuilder::default();
    let budget = make_budget(error_total, error_budget)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    let counts = InternalLogicalCounts::new(qubits, cx, ccx);
    let counts_py = LogicalCounts::from(&counts);
    let estimation = PhysicalResourceEstimation::new(qec, Rc::new(qubit), builder, Rc::new(counts));

    // Single best estimate
    let single_est: AliceAndBobEstimates = estimation
        .estimate(&budget)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
        .into();
    let single_report = EstimatesReport::from(&single_est);

    // Optional frontier
    let frontier_report = if frontier {
        let results = estimation
            .build_frontier(&budget)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Some(
            results
                .into_iter()
                .map(|r| {
                    let est: AliceAndBobEstimates = r.into();
                    EstimatesReport::from(&est)
                })
                .collect(),
        )
    } else {
        None
    };

    Ok(FullResults {
        estimates: single_report,
        frontier: frontier_report,
        counts: counts_py,
    })
}

#[pymethods]
impl EstimatesReport {
    fn __repr__(&self) -> String {
        format!("{self:?}")
    }
}

/// Runs the command-line interface against an explicit argument list.
///
/// `args` must start with a program name, as in `sys.argv`. Backs the
/// `anb-estimator` console script installed alongside the Python package, so
/// the CLI ships without a second compiled binary. Takes `sys.argv` from the
/// Python side rather than reading the process' `argv` directly, since the
/// two can differ once Python re-execs a script through its interpreter.
///
/// # Errors
/// Propagates any CLI failure as a Python `RuntimeError`.
#[pyfunction]
fn _cli_main(args: Vec<String>) -> PyResult<()> {
    crate::cli::run(args).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

/// Python module entry point for the Alice & Bob Q# resource estimator bindings.
///
/// Registers user-facing functions that load Q# programs, accept explicit logical counts,
/// in both pretty-printed and structured forms.
///
/// # Exposed callables
/// - `_estimate_qsharp_file(...)`
/// - `_estimate_logical_counts(...)`
/// - `_cli_main()`
///
/// # Errors
/// Any initialization failure is surfaced as a Python `RuntimeError`.
#[pymodule]
#[pyo3(name = "_native")]
fn anb_estimator(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
    // functions
    m.add_function(wrap_pyfunction!(_estimate_qsharp_file, m)?)?;
    m.add_function(wrap_pyfunction!(_estimate_logical_counts, m)?)?;
    m.add_function(wrap_pyfunction!(_cli_main, m)?)?;

    // classes
    m.add_class::<EstimatesReport>()?;
    m.add_class::<LogicalCounts>()?;
    m.add_class::<FullResults>()?;

    Ok(())
}
