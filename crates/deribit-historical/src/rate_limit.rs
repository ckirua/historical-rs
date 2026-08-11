use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

/// Simple async token-bucket rate limiter.
#[derive(Debug)]
pub struct TokenBucket {
    inner: Mutex<State>,
    rate_per_sec: f64,
    capacity: f64,
}

#[derive(Debug)]
struct State {
    tokens: f64,
    last: Instant,
}

impl TokenBucket {
    pub fn new(rate_per_sec: f64) -> Arc<Self> {
        let rate = rate_per_sec.max(0.1);
        Arc::new(Self {
            inner: Mutex::new(State {
                tokens: rate,
                last: Instant::now(),
            }),
            rate_per_sec: rate,
            capacity: rate.max(1.0),
        })
    }

    pub async fn acquire(&self) {
        loop {
            let sleep_for = {
                let mut state = self.inner.lock().await;
                let now = Instant::now();
                let elapsed = now.duration_since(state.last).as_secs_f64();
                state.tokens = (state.tokens + elapsed * self.rate_per_sec).min(self.capacity);
                state.last = now;
                if state.tokens >= 1.0 {
                    state.tokens -= 1.0;
                    None
                } else {
                    let need = 1.0 - state.tokens;
                    Some(Duration::from_secs_f64(need / self.rate_per_sec))
                }
            };
            match sleep_for {
                None => return,
                Some(delay) => tokio::time::sleep(delay.max(Duration::from_millis(1))).await,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn acquires_without_panic() {
        let bucket = TokenBucket::new(100.0);
        bucket.acquire().await;
        bucket.acquire().await;
    }
}
