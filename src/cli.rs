//! Command line interface to the resource estimator for cat-based quantum
//! computer with repetition code. The command-line is self documented, please
//! use it with subcommand `help` to learn its usage.
//!
//! Shared by the `anb_estimator_cli` binary and the Python console script
//! (`anb-estimator`, wired through [`crate::python`]).

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

use crate::estimates::make_budget;
use crate::{EstimatesReport, LogicalCounts, estimate_frontier, estimate_resources};
use resource_estimator::estimates::Overhead;

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

    /// Maximum number of physical qubits. Selects the fastest estimate that
    /// fits, which can use more qubits than the estimate without this option.
    /// Fails if no estimate fits.
    #[arg(long, value_name = "QUBITS")]
    max_physical_qubits: Option<u64>,

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

/// Parses `args` (program name followed by the actual arguments, as in
/// `std::env::args`) and runs the estimation.
///
/// Takes an explicit argument list, rather than reading the process' `argv`
/// directly, so that callers embedding this in another runtime (e.g. the
/// Python console script, via [`crate::python`]) can supply their own
/// accurate view of the command line instead of the host process' `argv`.
///
/// # Errors
/// Propagates I/O, parsing, or estimation failures.
pub fn run<I, T>(args: I) -> Result<(), anyhow::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Cli::parse_from(args);

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
    let num_magic_states = count
        .num_magic_states(&budget, 0)
        .map_err(anyhow::Error::msg)?;
    if num_magic_states == 0 {
        anyhow::bail!("The number of CCX gates must be > 0");
    }

    if args.frontier {
        let reports: Vec<EstimatesReport> =
            estimate_frontier(count, &budget, args.max_physical_qubits)?
                .iter()
                .map(EstimatesReport::from)
                .collect();
        for r in &reports {
            println!("{r}");
        }
        if let Some(path) = args.json {
            std::fs::write(path, serde_json::to_string_pretty(&reports)?)?;
        }
    } else {
        let result = estimate_resources(count, &budget, args.max_physical_qubits)?;
        let report = EstimatesReport::from(&result);
        println!("{report}");
        if let Some(path) = args.json {
            std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
        }
    }

    Ok(())
}
