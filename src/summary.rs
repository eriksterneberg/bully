use std::time::Duration;
use async_std::channel::Receiver;
use async_std::task::sleep;
use indicatif::{ProgressBar, ProgressStyle};
use prettytable::{Cell, Row, Table};
use tdigest::TDigest;
use crate::types::{HttpStatusCounter, Results};
use crate::types::Results::{Died, RequestDetails, Started, Stopped};
use crate::duration::DurationToF64;
use crate::parameters::Parameters;


/// Summarise the results of the benchmark
///
/// # Arguments
///   * `parameters` - The parameters used to run the program
///     * `report` - The channel to receive the results
pub async fn summarise(parameters: Parameters, report: Receiver<Results>) {
    let digest = TDigest::new_with_size(100);
    let mut values = Vec::with_capacity(parameters.requests);

    let mut status_counter = HttpStatusCounter::new();

    let bar = ProgressBar::new(parameters.requests as u64 - 1);
    bar.set_style(ProgressStyle::default_bar()
        .template("{spinner:.red} [{elapsed_precise}] [{bar:40.red/pink}] {percent}% {msg}").unwrap());
    let step = parameters.requests / 10;
    let mut count = 0;

    bar.inc(step as u64);
    let mut alive_workers: u32 = 0;
    let mut finished_workers: u32 = 0;
    let mut dead_workers: u32 = 0;
    let mut max_alive_workers: u32 = 0;
    bar.set_message(format!("Alive workers: {}; finished workers: {}; dead workers: {}", alive_workers, finished_workers, dead_workers));

    while let Ok(value) = report.recv().await {
        match value {
            RequestDetails { status_code, latency } => {
                values.push(latency.to_f64());
                status_counter.increment(status_code);

                count+=1;

                if count % step == 0 {
                    bar.inc(step as u64);
                }
            }
            Started => {
                alive_workers += 1;
            }
            Died => {
                alive_workers -= 1;
                dead_workers += 1;
            }
            Stopped => {
                finished_workers += 1;
                alive_workers -= 1;
            }
        }

        if alive_workers > max_alive_workers {
            max_alive_workers = alive_workers;
        }

        bar.set_message(format!("Working: alive workers: {}; finished workers: {}; dead workers: {}", alive_workers, finished_workers, dead_workers));
    }

    sleep(Duration::from_millis(1000)).await;

    bar.set_message(format!("Done! alive workers: {}; finished workers: {}; dead workers: {}", alive_workers, finished_workers, dead_workers));

    if max_alive_workers < parameters.concurrency as u32 {
        println!("Max alive workers: {}; expected: {}.", max_alive_workers, parameters.concurrency);
        println!("By running the program with export RUST_LOG=warn or lower, you can see the log messages for the workers that died.");
        println!("This can help you determine the cause of death. For instance, this error message might indicate that the number of file descriptors available is too low:");
        println!("    request completed with error: failed to connect to the server");
        println!();
        println!("If you see this error, try raising the ulimit.");
        println!("If you see a different error, please open an issue on the GitHub repository.");
        println!();
    };

    print_digest(parameters, digest.merge_unsorted(values), status_counter);
}



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
