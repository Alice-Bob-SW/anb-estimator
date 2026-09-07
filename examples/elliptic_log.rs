// Copyright (c) Microsoft Corporation.
// Copyright (c) Alice & Bob
// Licensed under the MIT License.

#![warn(missing_docs)]
//! Estimate the resources required for Elliptic Curve Cryptography (ECC) on a
//! cat-based quantum processor.
//!
//! Based on É. Gouzien et al.'s article (<https://arxiv.org/abs/2302.06639>)
//! and code (<https://github.com/ElieGouzien/elliptic_log_cat/tree/master>).

use std::rc::Rc;

use anb_estimator::{
    AliceAndBobEstimates, CatQubit, EstimatesReport, LogicalCounts, RepetitionCode, ToffoliBuilder,
};
use resource_estimator::estimates::{ErrorBudget, PhysicalResourceEstimation};

/// Compute logical qubits number and logical gates counts for elliptic curve
/// discrete logarithm computation, based on <https://arxiv.org/abs/2302.06639>.
#[allow(clippy::similar_names)]
fn elliptic_curve_crypto_count(bit_size: u64, window_size: u64) -> LogicalCounts {
    // Number of qubits for discrete log computation, arXiv:2302.06639 (p. 22, app C.11)
    let qubit_count = 9 * bit_size + window_size + 4;
    // Asymptotic gate counts, arXiv:2302.06639 (p. 21, app C.10)
    let cx_count = (448 * bit_size.pow(3)).div_ceil(window_size);
    let ccx_count = (348 * bit_size.pow(3)).div_ceil(window_size);

    LogicalCounts::new(qubit_count, cx_count, ccx_count)
}

/// Estimate resources for EC Shor algorithm from pre-computed counts.
fn main() -> Result<(), anyhow::Error> {
    // This value can be changed to investigate other key sizes, e.g., those in
    // arXiv:2302.06639 (Table IV, p. 37)
    let bit_size = 256;
    // Window size for modular exponentiation (arXiv:2001.09580, sec 4.1, p. 6)
    // Value w_e as reported in arXiv:2302.06639 (Table IV, p. 37)
    let window_size = 18;

    let qubit = CatQubit::new();
    let qec = RepetitionCode::new();
    let builder = ToffoliBuilder::default();
    let count = elliptic_curve_crypto_count(bit_size, window_size);
    let budget = ErrorBudget::new(0.333 * 0.5, 0.333 * 0.5, 0.0);

    let estimation =
        PhysicalResourceEstimation::new(qec, Rc::new(qubit), builder, Rc::new(count), budget);
    let result: AliceAndBobEstimates = estimation.estimate()?.into();
    let report = EstimatesReport::from(&result);
    println!("Estimates from pre-computed logical count (elliptic curve discrete logarithm):");
    println!("{report}");

    println!("----------------------------------------");
    println!("Exploration of good estimates from pre-computed logical count (elliptic curve discrete logarithm):");
    let results = estimation.build_frontier()?;

    for r in results {
        println!("{}", EstimatesReport::from(&AliceAndBobEstimates::from(r)));
    }

    Ok(())
}
