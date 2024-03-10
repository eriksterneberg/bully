mod types;

use std::thread::{sleep};
use std::time::Duration;
use clap::Parser;
use crossbeam_channel::bounded;
use indicatif::ProgressBar;
use tdigest::TDigest;
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

        let step = parameters.requests / 10;

        let mut count = 0;

        for msg in finished.iter() {
            digest = digest.merge_unsorted(vec![msg as f64]);
            count+=1;
            if count % step == 0 {
                bar.inc(step as u64);
            }
        }

        bar.finish_with_message("Finished.");

        print_digest(digest);

    }).unwrap();
}

fn send_request(id:i64, finish: crossbeam_channel::Sender<i64>) {
    sleep(Duration::from_millis(500));
    finish.send(id).unwrap();
}


fn print_digest(digest: TDigest) {
    let median = digest.estimate_quantile(0.5);
    let p90 = digest.estimate_quantile(0.9);
    let p99 = digest.estimate_quantile(0.99);

    let mut table = Table::new();

    table.add_row(Row::new(vec![
        Cell::new("Measurement"),
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
}