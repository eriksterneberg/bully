use prettytable::{Cell, Row, Table};
use tdigest::TDigest;
use crate::types::{HttpStatusCounter, Parameters};

/// Print the summary statistics of a TDigest
///
/// # Arguments
///    * `param` - The parameters used to run the program
///   * `digest` - The TDigest to summarise
pub fn print_digest(param: Parameters, digest: TDigest, http_status_counter: HttpStatusCounter) {
    let avg = format!("{:.precision$}", digest.mean(), precision = param.precision);
    let median = format!("{:.precision$}", digest.estimate_quantile(0.50), precision = param.precision);
    let p80 = format!("{:.precision$}", digest.estimate_quantile(0.80), precision = param.precision);
    let p90 = format!("{:.precision$}", digest.estimate_quantile(0.90), precision = param.precision);
    let p99 = format!("{:.precision$}", digest.estimate_quantile(0.99), precision = param.precision);
    let mut table = Table::new();

    table.add_row(Row::new(vec![
        Cell::new("Measurement"),
        Cell::new("Average"),
        Cell::new("Median"),
        Cell::new("p80"),
        Cell::new("p90"),
        Cell::new("p99"),
    ]));

    table.add_row(Row::new(vec![
        Cell::new("Latency (s)"),
        Cell::new(&avg),
        Cell::new(&median),
        Cell::new(&p80),
        Cell::new(&p90),
        Cell::new(&p99),
    ]));

    table.printstd();

    println!("HTTP Status Codes:");
    for (status, count) in http_status_counter.counter.iter() {
        println!("  * {}: {}", status, count);
    }
}
