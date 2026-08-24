// Copyright (c) Microsoft Corporation.
// Copyright (c) Alice & Bob
// Licensed under the MIT License.

#![warn(missing_docs)]
//! Estimate the resources required for an adder, the adder being specified by a
//! Q# file.

use std::rc::Rc;

use qsharp_alice_bob_resource_estimator::{
    AliceAndBobEstimates, CatQubit, EstimatesReport, LogicalCounts, RepetitionCode, ToffoliBuilder,
};
use resource_estimator::estimates::{ErrorBudget, PhysicalResourceEstimation};

/// Example of resource estimation from a Q# file.
fn main() -> Result<(), anyhow::Error> {
    let filename = format!("{}/qsharp/Adder.qs", env!("CARGO_MANIFEST_DIR"));

    let qubit = CatQubit::new();
    let qec = RepetitionCode::new();
    let builder = ToffoliBuilder::default();
    let count = LogicalCounts::from_qsharp(filename).map_err(anyhow::Error::msg)?;
    let budget = ErrorBudget::new(0.001 * 0.5, 0.001 * 0.5, 0.0);

    let estimation =
        PhysicalResourceEstimation::new(qec, Rc::new(qubit), builder, Rc::new(count), budget);
    let result: AliceAndBobEstimates = estimation.estimate()?.into();
    let report = EstimatesReport::from(&result);
    println!("Resource estimate from Q# code (ripple-carry adder):");
    println!("{report}");

    Ok(())
}
