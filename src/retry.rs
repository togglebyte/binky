use std::ops::Range;
use std::time::Duration;

use rand::prelude::*;

use crate::error::{Error, Result};

/// A timeout representation
/// ```
/// use std::time::Duration;
///
/// # async fn run_async() {
/// let timeout = binky::timeout()
///     .duration(Duration::from_millis(100))
///     .retries(10)
///     .sleep()
///     .await;
/// # }
/// ```
#[derive(Debug, Copy, Clone)]
pub struct Timeout {
    retries: RetryCount,
    sleep: Sleep,
    jitter: Option<(u64, u64)>,
}

impl Timeout {
    /// Sleeps for a given duration.
    ///
    /// This function is designed to be called repeatedly until it return `Error::NoRetry`
    pub async fn sleep(&mut self) -> Result<()> {
        match &mut self.retries {
            RetryCount::Count(0) | RetryCount::Never => return Err(Error::NoRetry),
            RetryCount::Count(count) => {
                *count -= 1;
            }
            RetryCount::Forever => {}
        }

        let mut duration = match &mut self.sleep {
            Sleep::NoSleep => return Err(Error::NoRetry),
            Sleep::Linear(duration, add) => {
                let d = *duration;
                *duration += *add;
                d
            }
            Sleep::Exponential(duration) => {
                let d = *duration;
                *duration *= 2;
                d
            }
            Sleep::Duration(duration) => *duration,
        };

        if let Some(jitter) = self.jitter.as_ref() {
            let mut rng = thread_rng();
            duration += Duration::from_millis(rng.gen_range(jitter.0..jitter.1));
        }

        tokio::time::sleep(duration).await;

        Ok(())
    }

    /// Set the jitter in milliseconds
    pub fn jitter_ms(mut self, range: Range<u64>) -> Self {
        self.jitter = Some((range.start, range.end));
        self
    }

    /// The sleep duration
    pub fn duration(mut self, duration: Duration) -> Self {
        self.sleep = Sleep::Duration(duration);
        self
    }

    /// The sleep duration in milliseconds
    pub fn duration_ms(self, millis: u64) -> Self {
        self.duration(Duration::from_millis(millis))
    }

    /// Linearly increment the duration by adding `add` after each sleep.
    pub fn linear(mut self, duration: Duration, add: Duration) -> Self {
        self.sleep = Sleep::Linear(duration, add);
        self
    }

    /// Exponentially increase the sleep time per sleep
    pub fn exponential(mut self, duration: Duration) -> Self {
        self.sleep = Sleep::Exponential(duration);
        self
    }

    /// This will prevent `sleep` from ever returning `Error::NoRetry`
    pub fn forever(mut self) -> Self {
        self.retries = RetryCount::Forever;
        self
    }

    /// Set the number of times `sleep` can be called before returning `Error::NoRetry`
    pub fn retries(mut self, count: usize) -> Self {
        self.retries = RetryCount::Count(count);
        self
    }
}

#[derive(Debug, Copy, Clone)]
pub enum RetryCount {
    Never,
    Count(usize),
    Forever,
}

#[derive(Debug, Copy, Clone)]
pub enum Sleep {
    NoSleep,
    Duration(Duration),
    Linear(Duration, Duration),
    Exponential(Duration),
}

/// Create a retry strategy
pub fn timeout() -> Timeout {
    Timeout {
        retries: RetryCount::Count(1),
        sleep: Sleep::NoSleep,
        jitter: None,
    }
}
