mod types;
mod summary;
mod duration;
mod parameters;

use std::ops::Add;
use std::time::{Duration, Instant};
use clap::Parser;
use futures::future::{join_all};
use crate::types::{Job};
use async_std::channel::{bounded};
use async_std::task;
use async_std::task::sleep;
use isahc::{
    prelude::*,
    HttpClient,
};
use crate::parameters::Parameters;
use crate::summary::{summarise};
use crate::types::Results::{Died, RequestDetails, Started, Stopped};

async fn worker(job: Job) {
    let mut slept = false;

    while job.jobs.recv().await.is_ok() {
        if !slept {
            // Slowly ramp up the number of concurrent requests
            job.sender.send(Started).await.expect("Failed to send report");
            sleep(job.delay).await;
            slept = true;
        }

        let t1 = Instant::now();
        let response = job.client.get_async(&job.path).await;
        let t2 = Instant::now();

        match response {
            Ok(mut response) => {
                let results = RequestDetails {
                    status_code: response.status().as_u16(),
                    latency: t2 - t1,
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

async fn main_() -> Result<(), isahc::Error> {
    let parameters = Parameters::parse();

    let (enqueue, jobs) = bounded(parameters.requests);
    let (report, results) = bounded(parameters.requests);

    let mut tasks = Vec::with_capacity(parameters.concurrency);
    tasks.push(task::spawn(summarise(parameters.clone(), results)));

    for _ in 0..parameters.requests {
        enqueue.send(true).await.unwrap();
    }

    let client = HttpClient::new()?;
    let mut delay= Duration::from_millis(0);

    for i in 0..parameters.concurrency {
        if i % 50 == 0 {
            delay = delay.add(Duration::from_millis(50));
        }

        let job = Job{
            client: client.clone(),
            path: parameters.path.clone(),
            delay,
            jobs: jobs.clone(),
            sender: report.clone(),
        };
        tasks.push(task::spawn(worker(job)));
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
