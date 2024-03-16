use std::time::Duration;

pub trait DurationToF64 {
    fn to_f64(&self) -> f64;
}

impl DurationToF64 for Duration{
    /// Convert a duration to a floating point number of seconds
    ///
    /// # Returns
    ///    * A floating point number of seconds
    fn to_f64(&self) -> f64 {
        // Convert duration to seconds as f64
        let seconds = self.as_secs() as f64;
        // Convert nanoseconds to seconds and add to the total
        let nanos = self.subsec_nanos() as f64 / 1_000_000_000.0;
        seconds + nanos
    }
}