// Copyright (c) Alice & Bob
// Licensed under the Apache License.

//! Command line interface to the resource estimator for cat-based quantum
//! computer with repetition code. The command-line is self documented, please
//! use it with subcommand `help` to learn its usage.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use std::rc::Rc;

use qsharp_alice_bob_resource_estimator::estimates::make_budget;
use qsharp_alice_bob_resource_estimator::{
    AliceAndBobEstimates, CatQubit, EstimatesReport, LogicalCounts, RepetitionCode, ToffoliBuilder,
};
use resource_estimator::estimates::PhysicalResourceEstimation;

/// Resource estimator for Alice & Bob's architecture (cats + repetition code).
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Show the frontier of good parameter sets instead of a single result.
    #[arg(short, long)]
    frontier: bool,

    /// Also write the computed estimate(s) as JSON to this file.
    #[arg(long, value_name = "FILE")]
    json: Option<PathBuf>,

    #[command(flatten)]
    budget: Budget,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Args)]
#[group(required = false, multiple = false)]
struct Budget {
    /// Overall error budget (equally split between topological and magic state
    /// errors) [default: 0.333].
    #[arg(long, value_name = "ERROR_PROBA")]
    error_total: Option<f64>,

    /// Detailed error budget
    #[arg(long, num_args = 3, value_names = ["TOPOLOGICAL_ERROR", "MAGIC_ERROR", "ROTATION_ERROR"])]
    error_budget: Option<Vec<f64>>,
}

#[derive(Subcommand)]
enum Commands {
    /// Read a Q# file
    File {
        /// Path to the Q# file
        filename: String,
    },
    /// Compute from listed resources
    Resources {
        /// Logical qubit number
        qubits: u64,
        /// Number of controlled-not gates
        cx: u64,
        /// Number of Toffoli gates
        ccx: u64,
    },
}

fn main() -> Result<(), anyhow::Error> {
    let args = Cli::parse();

    let qubit = CatQubit::new();
    let qec = RepetitionCode::new();
    let builder = ToffoliBuilder::default();
    let budget = make_budget(
        args.budget.error_total,
        args.budget.error_budget.map(|vec| (vec[0], vec[1], vec[2])),
    )
    .expect("Clap should have caught that!");

    let count = match args.command {
        Commands::File { filename } => {
            LogicalCounts::from_qsharp(filename).map_err(anyhow::Error::msg)?
        }
        Commands::Resources { qubits, cx, ccx } => LogicalCounts::new(qubits, cx, ccx),
    };
    let estimation =
        PhysicalResourceEstimation::new(qec, Rc::new(qubit), builder, Rc::new(count), budget);

    if args.frontier {
        let reports: Vec<EstimatesReport> = estimation
            .build_frontier()?
            .into_iter()
            .map(|r| EstimatesReport::from(&AliceAndBobEstimates::from(r)))
            .collect();
        for r in &reports {
            println!("{r}");
        }
        if let Some(path) = args.json {
            std::fs::write(path, serde_json::to_string_pretty(&reports)?)?;
        }
    } else {
        let result: AliceAndBobEstimates = estimation.estimate()?.into();
        let report = EstimatesReport::from(&result);
        println!("{report}");
        if let Some(path) = args.json {
            std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
        }
    }

    Ok(())
}
