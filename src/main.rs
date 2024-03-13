mod types;

use std::sync::Arc;
use std::time::{Duration, Instant};
use clap::Parser;
use futures::future::{join_all};
use tdigest::TDigest;
use prettytable::{Table, Row, Cell};
use crate::types::{ConnectionPool, HttpStatusCounter, Parameters};
use async_std::channel::{bounded, Receiver, Sender};
use async_std::sync::Mutex;
use async_std::task;
use indicatif::{ProgressBar, ProgressStyle};
use surf::Client;

async fn worker(client: Client, receiver: Receiver<bool>, sender: Sender<(Duration, surf::StatusCode)>) {

    while receiver.recv().await.is_ok() {

        let t1 = Instant::now();
        let response = client.get("http://localhost:8000/echo").await;
        let t2 = Instant::now();

        match response {
            Ok(response) => {
                sender.send((t2 - t1, response.status())).await.expect("Failed to send report");                // println!("Response: {}", response.status());
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
}

async fn main_() {
    let parameters = types::Parameters::parse();

    let (enqueue, jobs) = bounded(parameters.requests);
    let (report, results) = bounded(parameters.requests); // Bounded channel with capacity 1 to avoid unnecessary memory usage

    for _ in 0..parameters.requests {
        enqueue.send(true).await.unwrap();
    }

    let mut tasks = Vec::with_capacity(parameters.concurrency);


    // let connection_pool = Arc::new(Mutex::new(ConnectionPool::new(10)));

    let client = Client::new();

    for _ in 0..parameters.concurrency {
        tasks.push(task::spawn(worker(client.clone(), jobs.clone(), report.clone())));
    }

    drop(enqueue);
    drop(report);

    tasks.push(task::spawn(summarise(parameters, results)));

    join_all(tasks).await;
}


fn main() {
    task::block_on(main_());
}

/// Summarise the results of the benchmark
///
/// # Arguments
///   * `parameters` - The parameters used to run the program
///     * `report` - The channel to receive the results
async fn summarise(parameters: Parameters, report: Receiver<(Duration, surf::StatusCode)>) {

    // Todo: switch out TDigest for a library that supports incremental updates
    let digest = TDigest::new_with_size(100);
    let mut values = Vec::with_capacity(parameters.requests);

    let mut status_counter = HttpStatusCounter::new();

    let bar = ProgressBar::new(parameters.requests as u64);
    bar.set_style(ProgressStyle::default_bar()
        .template("{spinner:.red} [{elapsed_precise}] [{bar:40.red/pink}] {percent}% {msg}").unwrap());
    let step = parameters.requests / 10;
    let mut count = 0;

    while let Ok(value) = report.recv().await {
        values.push(duration_to_f64(value.0));
        status_counter.increment(value.1);
        count+=1;
        if count % step == 0 {
            bar.inc(step as u64);
        }
    }

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