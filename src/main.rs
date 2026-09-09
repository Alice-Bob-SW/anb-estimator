//! Binary entry point; see [`anb_estimator::cli`] for the actual CLI logic.

fn main() -> Result<(), anyhow::Error> {
    anb_estimator::cli::run(std::env::args_os())
}
