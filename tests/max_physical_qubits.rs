//! Checks the physical qubit cap of `estimate_resources` and `estimate_frontier`.

use anb_estimator::{
    AliceAndBobEstimates, EstimationError, LogicalCounts, estimate_frontier, estimate_resources,
};
use resource_estimator::estimates::ErrorBudget;

fn counts() -> LogicalCounts {
    LogicalCounts::new(100, 50_000, 20_000)
}

fn budget() -> ErrorBudget {
    ErrorBudget::new(0.001 * 0.5, 0.001 * 0.5, 0.0)
}

fn min_physical_qubits(frontier: &[AliceAndBobEstimates]) -> u64 {
    frontier
        .iter()
        .map(AliceAndBobEstimates::physical_qubits)
        .min()
        .expect("frontier is not empty")
}

#[test]
fn cap_at_uncapped_estimate_is_not_slower() {
    let uncapped = estimate_resources(counts(), &budget(), None).expect("should estimate");
    let cap = uncapped.physical_qubits();
    let capped = estimate_resources(counts(), &budget(), Some(cap)).expect("should fit");

    assert!(capped.physical_qubits() <= cap);
    assert!(capped.runtime() <= uncapped.runtime());
}

#[test]
fn capped_estimate_fits_and_gets_faster_with_more_qubits() {
    let frontier = estimate_frontier(counts(), &budget(), None).expect("should build frontier");
    let min = min_physical_qubits(&frontier);
    let max = frontier
        .iter()
        .map(AliceAndBobEstimates::physical_qubits)
        .max()
        .expect("frontier is not empty");
    assert!(min < max, "frontier should have several points");

    let mut last_runtime = u64::MAX;
    for cap in (min..=max).step_by(usize::try_from((max - min) / 20).unwrap_or(1).max(1)) {
        let estimate = estimate_resources(counts(), &budget(), Some(cap)).expect("should fit");
        assert!(estimate.physical_qubits() <= cap);
        assert!(estimate.runtime() <= last_runtime);
        last_runtime = estimate.runtime();
    }
}

#[test]
fn capped_frontier_only_keeps_fitting_estimates() {
    let frontier = estimate_frontier(counts(), &budget(), None).expect("should build frontier");
    let min = min_physical_qubits(&frontier);
    let cap = min + 1000;

    let capped = estimate_frontier(counts(), &budget(), Some(cap)).expect("should fit");
    assert!(capped.len() > 1, "frontier should have several points");
    assert!(capped.iter().all(|e| e.physical_qubits() <= cap));
    // Sorted by decreasing qubits, and each point is strictly slower than the
    // previous one, so no point dominates another.
    for pair in capped.windows(2) {
        assert!(pair[0].physical_qubits() > pair[1].physical_qubits());
        assert!(pair[0].runtime() < pair[1].runtime());
    }
}

fn reported_minimum(result: Result<(), EstimationError>, cap: u64) -> u64 {
    match result {
        Err(EstimationError::MaxPhysicalQubitsTooSmall {
            max_physical_qubits,
            min_physical_qubits: Some(min),
        }) => {
            assert_eq!(max_physical_qubits, cap);
            min
        }
        Err(e) => panic!("unexpected error: {e}"),
        Ok(()) => panic!("expected an error"),
    }
}

#[test]
fn cap_too_small_reports_minimum() {
    let min = reported_minimum(
        estimate_resources(counts(), &budget(), Some(1)).map(|_| ()),
        1,
    );
    assert_eq!(
        reported_minimum(
            estimate_frontier(counts(), &budget(), Some(min - 1)).map(|_| ()),
            min - 1
        ),
        min
    );
    let estimate = estimate_resources(counts(), &budget(), Some(min)).expect("should fit");
    assert_eq!(estimate.physical_qubits(), min);
}

#[test]
/// The underlying estimator's frontier ignores routing qubits. Here its
/// smallest estimate has 1785 physical qubits (d=9, 4 factories), while
/// d=11 with 1 factory fits in 1748.
fn cap_finds_estimates_outside_underlying_frontier() {
    let counts = LogicalCounts::new(40, 10_000, 10_000);
    let frontier = estimate_frontier(counts.clone(), &budget(), None).expect("should build");
    assert_eq!(min_physical_qubits(&frontier), 1785);

    let estimate = estimate_resources(counts, &budget(), Some(1784)).expect("should fit");
    assert_eq!(estimate.physical_qubits(), 1748);
}

#[test]
/// With one magic state, one factory run is always enough. Adding factories
/// beyond that cannot be faster, so the search must stop there.
fn cap_with_single_magic_state_terminates() {
    let counts = LogicalCounts::new(3, 0, 1);
    let estimate =
        estimate_resources(counts.clone(), &budget(), Some(100_000)).expect("should fit");
    assert_eq!(
        estimate
            .toffoli_factory_part()
            .expect("has a factory")
            .copies(),
        1
    );

    let min = reported_minimum(
        estimate_resources(counts, &budget(), Some(1)).map(|_| ()),
        1,
    );
    assert!(min <= estimate.physical_qubits());
}
