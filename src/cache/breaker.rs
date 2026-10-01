//! TG-1207: circuit breaker in front of every Redis command.
//!
//! Redis is an accelerator; the database stays the source of truth. Without a breaker a slow
//! Redis made every request pay the full command timeout once per Redis call (session read,
//! session populate, history version, history page, history populate — about 2.5 s at the
//! default 500 ms), so the UI looked frozen while Redis was "up". After one transport failure
//! the breaker opens: callers skip Redis immediately for [`OPEN_COOLDOWN`], then a single probe
//! decides whether to close again.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long Redis is bypassed after a transport failure before one probe is allowed.
pub(crate) const OPEN_COOLDOWN: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Admission {
    /// Breaker closed: run the command.
    Run,
    /// Cooldown elapsed: this caller is the single probe.
    Probe,
    /// Open, or another caller is probing: skip Redis now.
    Bypass,
}

#[derive(Clone, Copy, Debug)]
enum State {
    Closed,
    Open { until: Instant },
    Probing,
}

#[derive(Debug)]
struct Inner {
    state: State,
    last_failure: Option<Instant>,
}

#[derive(Clone, Debug)]
pub(crate) struct Breaker {
    inner: Arc<Mutex<Inner>>,
    cooldown: Duration,
}

impl Breaker {
    pub(crate) fn new(cooldown: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                state: State::Closed,
                last_failure: None,
            })),
            cooldown,
        }
    }

    pub(crate) fn admit(&self, now: Instant) -> Admission {
        let mut inner = self.lock();
        match inner.state {
            State::Closed => Admission::Run,
            State::Open { until } if now >= until => {
                inner.state = State::Probing;
                Admission::Probe
            }
            State::Open { .. } | State::Probing => Admission::Bypass,
        }
    }

    /// Records a command outcome. Returns `true` when this call changed the breaker between
    /// closed and open, so the caller logs once per transition instead of once per request.
    pub(crate) fn record(&self, now: Instant, ok: bool) -> bool {
        let mut inner = self.lock();
        let was_closed = matches!(inner.state, State::Closed);
        if ok {
            if matches!(inner.state, State::Probing) {
                inner.state = State::Closed;
                return true;
            }
            return false;
        }
        inner.last_failure = Some(now);
        inner.state = State::Open {
            until: now + self.cooldown,
        };
        was_closed
    }

    /// Whether cached data written before the last failure may still be served.
    ///
    /// A failure may have swallowed a cache invalidation (the history version `INCR`), so pages
    /// cached before it can be stale. Pages live at most `ttl`, so once the breaker is closed
    /// and `ttl` has passed since the last failure, every page that could have been stale has
    /// expired. Before that, readers go to the database.
    pub(crate) fn trusts_entries_older_than(&self, now: Instant, ttl: Duration) -> bool {
        let inner = self.lock();
        matches!(inner.state, State::Closed)
            && inner
                .last_failure
                .is_none_or(|failed_at| now.saturating_duration_since(failed_at) >= ttl)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COOLDOWN: Duration = Duration::from_secs(5);

    #[test]
    fn one_failure_bypasses_until_cooldown_then_admits_a_single_probe() {
        let breaker = Breaker::new(COOLDOWN);
        let start = Instant::now();
        assert_eq!(breaker.admit(start), Admission::Run);
        assert!(
            breaker.record(start, false),
            "closed → open is a transition"
        );

        let during = start + Duration::from_secs(1);
        assert_eq!(breaker.admit(during), Admission::Bypass);
        assert_eq!(breaker.admit(during), Admission::Bypass);

        let after = start + COOLDOWN;
        assert_eq!(breaker.admit(after), Admission::Probe);
        assert_eq!(
            breaker.admit(after),
            Admission::Bypass,
            "only one caller probes at a time"
        );
        assert!(breaker.record(after, true), "probe success closes");
        assert_eq!(breaker.admit(after), Admission::Run);
    }

    #[test]
    fn a_failed_probe_reopens_for_another_cooldown() {
        let breaker = Breaker::new(COOLDOWN);
        let start = Instant::now();
        breaker.record(start, false);
        let probe_at = start + COOLDOWN;
        assert_eq!(breaker.admit(probe_at), Admission::Probe);
        assert!(
            !breaker.record(probe_at, false),
            "still open: no new transition"
        );
        assert_eq!(breaker.admit(probe_at + COOLDOWN / 2), Admission::Bypass);
        assert_eq!(breaker.admit(probe_at + COOLDOWN), Admission::Probe);
    }

    #[test]
    fn cached_entries_are_untrusted_for_one_ttl_after_any_failure() {
        let breaker = Breaker::new(COOLDOWN);
        let ttl = Duration::from_secs(30);
        let start = Instant::now();
        assert!(breaker.trusts_entries_older_than(start, ttl));

        breaker.record(start, false);
        assert!(
            !breaker.trusts_entries_older_than(start + ttl, ttl),
            "still open"
        );
        let probe_at = start + COOLDOWN;
        breaker.admit(probe_at);
        breaker.record(probe_at, true);
        assert!(
            !breaker.trusts_entries_older_than(probe_at, ttl),
            "closed again, but a lost invalidation may still be cached"
        );
        assert!(breaker.trusts_entries_older_than(start + ttl, ttl));
    }
}
