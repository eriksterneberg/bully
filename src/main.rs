use std::thread::{sleep};
use crossbeam_channel::bounded;

fn main() {
    let n_workers = 10;
    let n_jobs = 10000;

    let (schedule, job_queue) = bounded(1);
    let (finish, finished) = bounded(1);

    crossbeam::scope(|s| {
        s.spawn(move |_| {
            for _ in 0..n_jobs {
                schedule.send("some job").unwrap();
            }
            drop(schedule);
        });

        for i in 0..n_workers {
            let (finish_, job_queue_) = (finish.clone(), job_queue.clone());
            s.spawn(move |_| {
                for _ in job_queue_.iter() {
                    sleep(std::time::Duration::from_secs(i + 1));
                    let msg = format!("Worker thread {} finished", i);
                    finish_.send(msg).unwrap();
                }
            });
        }

        drop(finish);

        // Sink
        s.spawn(|_| {
            for msg in finished.iter() {
                println!("Got a message: {}", msg);
            }
        });
    }).unwrap();
}
