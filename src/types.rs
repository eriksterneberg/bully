use std::collections::HashMap;
use clap::Parser;


/// This program uses the standard flag RUST_LOG to set the log level.
/// Running the program with a log level higher than Info will result in an error.
#[derive(Clone, Parser)]
pub struct Parameters {
    #[arg(short = 'r', long = "requests", default_value = "100")]
    pub requests: usize,

    #[arg(short = 'c', long = "concurrency", default_value = "10")]
    pub concurrency: usize,

    #[arg(short = 'p', long = "precision", default_value = "7")]
    pub precision: usize,
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

    pub fn get_count(&self, status_code: u16) -> Option<&u64> {
        self.counter.get(&status_code)
    }
}
