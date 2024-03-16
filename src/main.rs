mod types;
mod summary;
mod duration;
mod parameters;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use clap::Parser;
use futures::future::{join_all};
use anyhow::{Result};
use crate::types::{Job};
use async_std::channel::{bounded};
use async_std::task;
use async_std::task::{sleep, spawn};
use isahc::{
    prelude::*,
    HttpClient,
};
use crate::parameters::Parameters;
use crate::summary::{summarise};
use crate::types::Results::{Died, RequestDetails, Started, Stopped};


async fn worker(job: Job, running: Arc<AtomicBool>) {
    let mut slept = false;

    while job.jobs.recv().await.is_ok() {
        if !slept {
            // Slowly ramp up the number of concurrent requests
            job.sender.send(Started).await.expect("Failed to send report");
            sleep(job.delay).await;
            slept = true;
        }

        match check_running(&running) {
            Ok(_) => {}
            Err(_) => {
                job.sender.send(Stopped).await.expect("Failed to send report");
                return;
            }
        }

        let t1 = Instant::now();
        let response = job.client.get_async(&job.path).await;

        match response {
            Ok(mut response) => {
                let results = RequestDetails {
                    status_code: response.status().as_u16(),
                    latency: t1.elapsed(),
                };
                job.sender.send(results).await.expect("Failed to send report");
                let _ = response.consume().await;
            }
            Err(_) => {
                job.sender.send(Died).await.expect("Failed to send report");
                return;
            }
        }
    }

    job.sender.send(Stopped).await.expect("Failed to send report");
}

async fn main_() -> Result<()> {
    let parameters = Parameters::parse();
    let running = get_cancel();

    let (enqueue, jobs) = bounded(parameters.requests);
    let (report, results) = bounded(parameters.requests);

    let mut tasks = Vec::with_capacity(parameters.concurrency + 2);

    for _ in 0..parameters.requests {
        enqueue.send(true).await.unwrap();
    }

    let client = HttpClient::new().unwrap();

    for i in 0..parameters.concurrency {
        let worker_ = worker(Job{
            client: client.clone(),
            path: parameters.path.clone(),
            delay: Duration::from_millis(i as u64),
            jobs: jobs.clone(),
            sender: report.clone(),
        }, running.clone());
        tasks.push(spawn(worker_));
    }

    drop(enqueue);
    drop(report);

    tasks.push(spawn(summarise(parameters.clone(), results)));

    join_all(tasks).await;

    Ok(())
}


fn main() {
    env_logger::init();
    let _ = task::block_on(main_());
}


/// Creates a cancellation flag that can be used to gracefully terminate the program in response to a SIGINT signal (Ctrl+C).
///
/// This function sets up a signal handler for SIGINT signals using the `ctrlc` crate. When a SIGINT signal is received,
/// the `running` flag is set to `false`, indicating that the program should terminate gracefully.
///
/// # Returns
///
/// * `Arc<AtomicBool>` - Returns an `Arc<AtomicBool>` representing the cancellation flag.
fn get_cancel() -> Arc<AtomicBool> {
    let (tx, _rx) = crossbeam_channel::bounded(1);
    let running = Arc::new(AtomicBool::new(true));
    let running_ = running.clone();
    ctrlc::set_handler(move || {
        running_.store(false, Ordering::SeqCst);
        let _ = tx.send(());
    }).expect("Error setting Ctrl-C handler");
    running
}

fn check_running(running: &Arc<AtomicBool>) -> Result<()> {
    if !running.load(Ordering::SeqCst) {
        Err(anyhow::anyhow!("Cancelled"))
    } else {
        Ok(())
    }
}