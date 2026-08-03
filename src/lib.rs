#![warn(missing_docs)]
//! Resource estimator for a cat-based quantum processor using repetition code
//! and preparation of Toffoli magical states by fault-tolerant measurement.
//!
//! Hypothesis on the architecture, hardware and code performances are based on
//! É. Gouzien et al.'s article (<https://arxiv.org/abs/2302.06639>) and code
//! (<https://github.com/ElieGouzien/elliptic_log_cat/tree/master>).
//!
//! ### Notations:
//! - κ₁: one photon loss rate
//! - κ₂: two photons loss rate
//! - |α|²: average number of photons
//!
//! ### Assumes:
//! - architecture as described in
//!   [arXiv:2302.06639](https://arxiv.org/abs/2302.06639)
//! - 1/κ₂ = 100 ns
//! - κ₁/κ₂ = 1e-5
//! - no saturation of bit-flip
//! - simplified gate counting, when translating from Q# (no consequences for
//!   modular arithmetic circuits, approximation in general):
//!   * 1-qubit Clifford gates are free
//!   * CX, CY, CZ are count as CX
//!   * no parallelism considered
//!
//! ### Takes:
//! - specification of the algorithmic required resources, either entered
//!   directly, either deduced from a Q# file (see `example/from_qsharp.rs`).
//!   * number of logical qubits
//!   * number of logical CX
//!   * number of logical CCX
//! - error budget:
//!   * maximum total topological error probability
//!   * maximum total error probability from magic states preparations
//!   * maximum total error probability from rotations (unused)
//!
//! ### Provides:
//! - number of physical cat qubits
//! - runtime
//! - total error probability
//! - Code parameters:
//!     * repetition code distance
//!     * average number of photons |α|² in each cat
//! - fraction of qubits assigned to the magic state factory
//!
//! *Author: Mathias Soeken*

pub use code::RepetitionCode;
pub use counter::LogicalCounts;
pub use estimates::AliceAndBobEstimates;
#[cfg(any(feature = "cli", feature = "python"))]
pub use estimates::EstimatesReport;
pub use factories::ToffoliBuilder;
pub use qubit::CatQubit;

pub mod code;
pub mod counter;
pub mod ecc_example;
pub mod estimates;
pub mod factories;
pub mod gates;
pub mod hardware;
pub mod logical_utils;
pub mod qubit;

#[cfg(feature = "cli")]
pub mod cli;

// Only compile the Python module when the "python" feature is enabled.
#[cfg(feature = "python")]
mod python;
