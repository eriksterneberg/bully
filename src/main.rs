mod types;

use std::time::{Duration, Instant};
use clap::Parser;
use futures::future::{join_all};
use tdigest::TDigest;
use prettytable::{Table, Row, Cell};
use crate::types::{HttpStatusCounter, Parameters, Results};
use async_std::channel::{bounded, Receiver, Sender};
use async_std::task;
use async_std::task::sleep;
use indicatif::{ProgressBar, ProgressStyle};
use isahc::{
    prelude::*,
    HttpClient,
};
use crate::types::Results::{Died, RequestDetails, Started, Stopped};

async fn worker(duration: Duration, client: HttpClient, receiver: Receiver<bool>, sender: Sender<Results>) {
    let mut slept = false;

    while receiver.recv().await.is_ok() {
        if !slept {
            // Slowly ramp up the number of concurrent requests
            sender.send(Started).await.expect("Failed to send report");
            sleep(duration).await;
            slept = true;
        }

        let t1 = Instant::now();
        let response = client.get_async("http://localhost:8000/echo").await;
        let t2 = Instant::now();

        match response {
            Ok(mut response) => {
                let results = RequestDetails {
                    status_code: response.status().as_u16(),
                    latency: t2 - t1,
                };
                sender.send(results).await.expect("Failed to send report");
                let _ = response.consume().await;
            }
            Err(_) => {
                sender.send(Died).await.expect("Failed to send report");
                return;
            }
        }
    }

    sender.send(Stopped).await.expect("Failed to send report");
}

async fn main_() -> Result<(), isahc::Error> {
    let parameters = types::Parameters::parse();

    let (enqueue, jobs) = bounded(parameters.requests);
    let (report, results) = bounded(parameters.requests);

    let mut tasks = Vec::with_capacity(parameters.concurrency);
    tasks.push(task::spawn(summarise(parameters.clone(), results)));

    for _ in 0..parameters.requests {
        enqueue.send(true).await.unwrap();
    }

    let client = HttpClient::new()?;

    let mut start = 100;

    for i in 0..parameters.concurrency {
        if i % 50 == 0 {
            start += 200;
        }

        tasks.push(task::spawn(worker(Duration::from_millis(start as u64), client.clone(), jobs.clone(), report.clone())));
    }

    drop(enqueue);
    drop(report);

    join_all(tasks).await;

    Ok(())
}


fn main() {
    env_logger::init();
    let _ = task::block_on(main_());
}

/// Summarise the results of the benchmark
///
/// # Arguments
///   * `parameters` - The parameters used to run the program
///     * `report` - The channel to receive the results
async fn summarise(parameters: Parameters, report: Receiver<Results>) {
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
                values.push(duration_to_f64(latency));
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
fn print_digest(param: Parameters, digest: TDigest, http_status_counter: HttpStatusCounter) {
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

/// Convert a duration to a floating point number of seconds
///
/// # Arguments
///     * `duration` - The duration to convert
///
/// # Returns
///    * A floating point number of seconds
fn duration_to_f64(duration: Duration) -> f64 {
    // Convert duration to seconds as f64
    let seconds = duration.as_secs() as f64;
    // Convert nanoseconds to seconds and add to the total
    let nanos = duration.subsec_nanos() as f64 / 1_000_000_000.0;
    seconds + nanos
}