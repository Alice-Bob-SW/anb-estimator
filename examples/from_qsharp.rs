#![warn(missing_docs)]
//! Estimate the resources required for an adder, the adder being specified by a
//! Q# file.

use anb_estimator::{EstimatesReport, LogicalCounts, estimate_resources};
use resource_estimator::estimates::ErrorBudget;

/// Example of resource estimation from a Q# file.
fn main() -> Result<(), anyhow::Error> {
    let filename = format!("{}/qsharp/Adder.qs", env!("CARGO_MANIFEST_DIR"));

    let count = LogicalCounts::from_qsharp(filename).map_err(anyhow::Error::msg)?;
    let budget = ErrorBudget::new(0.001 * 0.5, 0.001 * 0.5, 0.0);

    let result = estimate_resources(count, &budget, None)?;
    let report = EstimatesReport::from(&result);
    println!("Resource estimate from Q# code (ripple-carry adder):");
    println!("{report}");

    Ok(())
}
