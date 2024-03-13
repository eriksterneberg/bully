use clap::Parser;

/// This program uses the standard flag RUST_LOG to set the log level.
/// Running the program with a log level higher than Info will result in an error.
#[derive(Clone, Parser)]
pub struct Parameters {
    #[arg(short = 'r', long = "requests", default_value = "100")]
    pub requests: usize,

    #[arg(short = 'c', long = "concurrency", default_value = "10")]
    pub concurrency: usize,

    #[arg(short = 'p', long = "precision", default_value = "7")]
    pub precision: usize,
}
