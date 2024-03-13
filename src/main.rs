mod types;

use std::time::{Duration, Instant};
use clap::Parser;
use futures::future::{join_all};
use tdigest::TDigest;
use prettytable::{Table, Row, Cell};
use crate::types::Parameters;
use async_std::channel::{bounded, Receiver, Sender};
use async_std::task;
use indicatif::{ProgressBar, ProgressStyle};
use surf::Client;

async fn worker(receiver: Receiver<bool>, sender: Sender<Duration>) {
    let client = Client::new();

    while receiver.recv().await.is_ok() {

        let start_time = Instant::now();
        let response = client.get("http://localhost:8000/echo").await;
        let end_time = Instant::now();

        match response {
            Ok(_) => {
                // println!("Response: {}", response.status());
            }
            Err(_) => {
                // println!("Error: {}", e);
            }
        }

        sender.send(end_time - start_time).await.expect("Failed to send report");
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

    for _ in 0..parameters.concurrency {
        tasks.push(task::spawn(worker(jobs.clone(), report.clone())));
    }

    drop(enqueue);
    drop(report);

    tasks.push(task::spawn(summarise(parameters, results)));

    join_all(tasks).await;
}


fn main() {
    task::block_on(main_());
}

async fn summarise(parameters: Parameters, report: Receiver<Duration>) {
    let digest = TDigest::new_with_size(100);
    let mut values = vec![];
    let bar = ProgressBar::new(parameters.requests as u64);
    bar.set_style(ProgressStyle::default_bar()
        .template("{spinner:.red} [{elapsed_precise}] [{bar:40.red/pink}] {percent}% {msg}").unwrap());
    let step = parameters.requests / 10;
    let mut count = 0;

    while let Ok(value) = report.recv().await {
        values.push(duration_to_f64(value));
        count+=1;
        if count % step == 0 {
            bar.inc(step as u64);
        }
    }

    print_digest(parameters, digest.merge_unsorted(values))
}

fn print_digest(param: Parameters, digest: TDigest) {
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
}

fn duration_to_f64(duration: Duration) -> f64 {
    // Convert duration to seconds as f64
    let seconds = duration.as_secs() as f64;
    // Convert nanoseconds to seconds and add to the total
    let nanos = duration.subsec_nanos() as f64 / 1_000_000_000.0;
    seconds + nanos
}