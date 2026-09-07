// Copyright (c) Microsoft Corporation.
// Copyright (c) Alice & Bob.
// Licensed under the MIT License.

//! End-to-end checks that the resource estimator's output stays consistent
//! with arXiv:2302.06639 for the two scenarios also demonstrated in
//! `examples/elliptic_log.rs` and `examples/from_qsharp.rs`.

use std::rc::Rc;

use anb_estimator::{
    code::CodeParameter, AliceAndBobEstimates, CatQubit, EstimatesReport, LogicalCounts,
    RepetitionCode, ToffoliBuilder,
};
use resource_estimator::estimates::{ErrorBudget, PhysicalResourceEstimation};

fn assert_close(actual: f64, expected: f64, relative_tolerance: f64) {
    assert!(
        (actual - expected).abs() <= relative_tolerance * expected.abs(),
        "expected {expected}, got {actual}"
    );
}

/// Qubit and gate counts for elliptic curve discrete logarithm
/// (arXiv:2302.06639, app C.10/C.11). Duplicated from
/// `examples/elliptic_log.rs`.
#[allow(clippy::similar_names)]
fn ecdlp_counts(bit_size: u64, window_size: u64) -> (u64, u64, u64) {
    let qubit_count = 9 * bit_size + window_size + 4;
    let cx_count = (448 * bit_size.pow(3)).div_ceil(window_size);
    let ccx_count = (348 * bit_size.pow(3)).div_ceil(window_size);
    (qubit_count, cx_count, ccx_count)
}

#[test]
/// Checks the "logical qubits" column of arXiv:2302.06639 Table IV (p. 37):
/// (n, `w_e`) = (8, 9) gives 85; (16, 11) gives 159; (256, 18) gives 2326.
fn qubit_count_matches_table_iv() {
    assert_eq!(ecdlp_counts(8, 9).0, 85);
    assert_eq!(ecdlp_counts(16, 11).0, 159);
    assert_eq!(ecdlp_counts(256, 18).0, 2326);
}

#[test]
#[allow(clippy::similar_names)]
/// Locks the app C.10 gate-count formula. Table IV has no gate-count
/// column to check against.
fn gate_counts_formula() {
    let (_, cx_count, ccx_count) = ecdlp_counts(256, 18);
    assert_eq!(cx_count, 417_566_265);
    assert_eq!(ccx_count, 324_359_510);
}

fn estimate(count: LogicalCounts, budget: ErrorBudget) -> EstimatesReport {
    let estimation = PhysicalResourceEstimation::new(
        RepetitionCode::new(),
        Rc::new(CatQubit::new()),
        ToffoliBuilder::default(),
        Rc::new(count),
        budget,
    );
    let result: AliceAndBobEstimates = estimation
        .estimate()
        .expect("estimation should succeed")
        .into();
    EstimatesReport::from(&result)
}

#[test]
/// Uses the paper's own parameters directly (arXiv:2302.06639 Table IV,
/// p. 37): code distance 13, `alpha^2`=19, Table III factory i=12 (d=19,
/// `alpha^2`=17.35), 84 factory copies.
///
/// Matches the paper's code distance and `alpha^2` exactly. Matches its
/// physical qubit count (126,133) within 0.1%. Matches its runtime (7h,
/// 9h expected) within 10%.
#[allow(clippy::similar_names)]
fn ecdlp_256_bit_matches_paper_at_its_own_parameters() {
    let (qubit_count, cx_count, ccx_count) = ecdlp_counts(256, 18);
    let count = LogicalCounts::new(qubit_count, cx_count, ccx_count);

    // Table III factory i=12: d=19, alpha^2=17.35.
    let factory = ToffoliBuilder::default().factories()[12].clone();
    let result = AliceAndBobEstimates::from_fixed_parameters(
        count,
        CodeParameter::new(13, 19.0),
        factory,
        84,
    )
    .expect("should build estimate");
    let report = EstimatesReport::from(&result);

    assert_eq!(report.physical_qubits, 126_248); // 126,133 in paper
    assert_eq!(report.runtime_hours, 7.573722061388889); // 7h in paper
    assert_eq!(report.total_error, 0.15900410163750558); // not in paper, just for regression test
    let expected_runtime_hours = report.runtime_hours / (1.0 - report.total_error);
    assert_close(expected_runtime_hours, 9.0, 0.01);
}

#[test]
/// Mirrors `examples/from_qsharp.rs` (ripple-carry adder from
/// `qsharp/Adder.qs`). Locks today's computed values; the paper does not
/// cover ripple-carry adders.
fn ripple_carry_adder_from_qsharp_is_stable() {
    let filename = format!("{}/qsharp/Adder.qs", env!("CARGO_MANIFEST_DIR"));
    let count = LogicalCounts::from_qsharp(filename).expect("should read Q# file");
    let budget = ErrorBudget::new(0.001 * 0.5, 0.001 * 0.5, 0.0);
    let report = estimate(count, budget);

    assert_eq!(report.physical_qubits, 13_419);
    assert_eq!(report.code_distance, 9);
    assert!((report.code_alpha2 - 14.0).abs() < 1e-9);
    assert_eq!(report.factories, 2);
    assert_eq!(report.factories_distance, 5);
    assert!((report.factories_alpha2 - 8.18).abs() < 1e-9);
    assert_close(report.total_error, 0.000_193_407_374_140_844_32, 1e-9);
    assert_close(report.runtime_seconds, 0.013_297_5, 1e-6);
}
