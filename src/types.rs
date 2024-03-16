use std::collections::HashMap;
use std::time::Duration;
use async_std::channel::{Receiver, Sender};
use isahc::HttpClient;

#[derive(Debug)]
pub enum Results {
    Started,
    Stopped,
    Died,
    RequestDetails{
        status_code: u16,
        latency: Duration,
    }
}


pub struct HttpStatusCounter {
    pub(crate) counter: HashMap<u16, u64>,
}

impl HttpStatusCounter {
    pub fn new() -> Self {
        HttpStatusCounter {
            counter: HashMap::new(),
        }
    }

    pub fn increment(&mut self, status_code: u16) {
        let count = self.counter.entry(status_code).or_insert(0);
        *count += 1;
    }
}


pub struct Job {
    pub client: HttpClient,
    pub path: String,
    pub delay: Duration,
    pub jobs: Receiver<bool>,
    pub sender: Sender<Results>,
}