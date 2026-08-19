use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use uuid::Uuid;

const SEARCH_WINDOW: Duration = Duration::from_secs(60);
const SEARCH_LIMIT: usize = 30;
const PAIR_WINDOW: Duration = Duration::from_secs(60);

#[derive(Default)]
struct RateLimitState {
    searches: HashMap<Uuid, VecDeque<Instant>>,
    pair_requests: HashMap<(Uuid, Uuid), Instant>,
}

#[derive(Default)]
pub(crate) struct SocialRateLimits {
    state: Mutex<RateLimitState>,
}

impl SocialRateLimits {
    pub async fn allow_search(&self, user_id: Uuid) -> bool {
        let now = Instant::now();
        let mut state = self.state.lock().await;
        let attempts = state.searches.entry(user_id).or_default();
        while attempts
            .front()
            .is_some_and(|attempt| now.duration_since(*attempt) >= SEARCH_WINDOW)
        {
            attempts.pop_front();
        }
        if attempts.len() >= SEARCH_LIMIT {
            return false;
        }
        attempts.push_back(now);
        true
    }

    pub async fn allow_new_pair_request(&self, direction: (Uuid, Uuid)) -> bool {
        let now = Instant::now();
        let mut state = self.state.lock().await;
        state
            .pair_requests
            .retain(|_, attempt| now.duration_since(*attempt) < PAIR_WINDOW);
        if state.pair_requests.contains_key(&direction) {
            return false;
        }
        state.pair_requests.insert(direction, now);
        true
    }

    pub async fn clear_pair_request(&self, pair: (Uuid, Uuid)) {
        let mut state = self.state.lock().await;
        state.pair_requests.remove(&pair);
        state.pair_requests.remove(&(pair.1, pair.0));
    }
}
