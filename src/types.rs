use std::collections::HashMap;
use clap::Parser;
use surf::Client;

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
    pub(crate) counter: HashMap<surf::StatusCode, u64>,
}

impl HttpStatusCounter {
    pub fn new() -> Self {
        HttpStatusCounter {
            counter: HashMap::new(),
        }
    }

    pub fn increment(&mut self, status_code: surf::StatusCode) {
        let count = self.counter.entry(status_code).or_insert(0);
        *count += 1;
    }

    pub fn get_count(&self, status_code: surf::StatusCode) -> Option<&u64> {
        self.counter.get(&status_code)
    }
}


// Define a struct to represent the connection pool
pub struct ConnectionPool {
    clients: Vec<Client>,
}

impl ConnectionPool {
    // Create a new connection pool with a specified number of clients
    pub fn new(pool_size: usize) -> Self {
        let mut clients = Vec::with_capacity(pool_size);
        for _ in 0..pool_size {
            clients.push(Client::new());
        }
        ConnectionPool { clients }
    }

    // Borrow an HTTP client from the pool
    pub fn borrow_client(&self) -> Option<&Client> {
        self.clients.first()
    }

    // Return an HTTP client to the pool
    pub fn return_client(&self, _client: &Client) {
        // Optionally perform cleanup or validation here
        // For example, reset headers, clear cookies, etc.
    }
}