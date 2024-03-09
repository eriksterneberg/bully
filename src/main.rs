mod types;

use std::fmt::format;
use std::thread::{sleep};
use clap::Parser;
use crossbeam_channel::bounded;
use tdigest::TDigest;
use indicatif::{ProgressBar};
use prettytable::{Table, Row, Cell};

fn main() {
    let parameters = types::Parameters::parse();

    let (schedule, job_queue) = bounded(1);
    let (finish, finished) = bounded(1);

    crossbeam::scope(|s| {
        s.spawn(move |_| {
            (0..parameters.requests).for_each(|_| schedule.send("some job").unwrap());
        });

        for i in 0..parameters.concurrency {
            let (finish_, job_queue_) = (finish.clone(), job_queue.clone());
            s.spawn(move |_| {
                job_queue_.iter().for_each(|_| send_request(i, finish_.clone()));
            });
        }

        drop(finish);

        // Sink
        let mut digest = TDigest::new_with_size(100);

        let bar = ProgressBar::new(parameters.requests as u64);

        for msg in finished.iter() {
            digest = digest.merge_unsorted(vec![msg as f64]);
            bar.inc(1);
        }

        let median = digest.estimate_quantile(0.5);
        let p90 = digest.estimate_quantile(0.9);
        let p99 = digest.estimate_quantile(0.99);

        let mut table = Table::new();

        table.add_row(Row::new(vec![
            Cell::new("Measurement"),
            // Cell::New("Avg"),
            Cell::new("Median"),
            Cell::new("p90"),
            Cell::new("p99"),
        ]));

        table.add_row(Row::new(vec![
           Cell::new("Latency (s)"),
            Cell::new(&median.to_string()),
           Cell::new(&p90.to_string()),
           Cell::new(&p99.to_string()),
        ]));

        table.printstd();

    }).unwrap();
}

fn send_request(id:i64, finish: crossbeam_channel::Sender<i64>) {
    sleep(std::time::Duration::from_secs(id as u64 + 1));
    finish.send(id).unwrap();
}
