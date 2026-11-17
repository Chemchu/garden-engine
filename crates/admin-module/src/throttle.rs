use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
struct Window {
    attempts: u32,
    started: Instant,
}

impl Window {
    fn new(now: Instant) -> Window {
        Window {
            attempts: 0,
            started: now,
        }
    }

    fn is_expired(&self, now: Instant, length: Duration) -> bool {
        now.saturating_duration_since(self.started) >= length
    }
}

struct State {
    clients: HashMap<String, Window>,
    global: Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Allowed { delay: Duration },
    Blocked,
}

pub struct LoginThrottle {
    state: Mutex<State>,
    per_client_limit: u32,
    global_limit: u32,
    slowdown: Duration,
    window: Duration,
}

impl LoginThrottle {
    pub fn new(
        per_client_limit: u32,
        global_limit: u32,
        slowdown: Duration,
        window: Duration,
    ) -> LoginThrottle {
        LoginThrottle {
            state: Mutex::new(State {
                clients: HashMap::new(),
                global: Window::new(Instant::now()),
            }),
            per_client_limit,
            global_limit,
            slowdown,
            window,
        }
    }

    pub fn admit(&self, client: &str, now: Instant) -> Admission {
        let mut state = self.lock();
        self.expire(&mut state, now);

        let window = state
            .clients
            .entry(client.to_string())
            .or_insert_with(|| Window::new(now));
        if window.attempts >= self.per_client_limit {
            return Admission::Blocked;
        }
        window.attempts += 1;

        let delay = if state.global.attempts >= self.global_limit {
            self.slowdown
        } else {
            Duration::ZERO
        };
        state.global.attempts = state.global.attempts.saturating_add(1);
        Admission::Allowed { delay }
    }

    pub fn record_success(&self, client: &str) {
        let mut state = self.lock();
        state.clients.remove(client);
        state.global.attempts = state.global.attempts.saturating_sub(1);
    }

    fn expire(&self, state: &mut State, now: Instant) {
        if state.global.is_expired(now, self.window) {
            state.global = Window::new(now);
        }
        state
            .clients
            .retain(|_, window| !window.is_expired(now, self.window));
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Duration = Duration::from_secs(900);
    const SLOWDOWN: Duration = Duration::from_secs(2);
    const ALLOWED: Admission = Admission::Allowed {
        delay: Duration::ZERO,
    };
    const SLOWED: Admission = Admission::Allowed { delay: SLOWDOWN };

    fn throttle(per_client: u32, global: u32) -> LoginThrottle {
        LoginThrottle::new(per_client, global, SLOWDOWN, WINDOW)
    }

    fn attempt(throttle: &LoginThrottle, client: &str, times: u32, now: Instant) {
        for _ in 0..times {
            assert_ne!(throttle.admit(client, now), Admission::Blocked);
        }
    }

    #[test]
    fn attempts_are_counted_before_they_are_checked() {
        let throttle = throttle(3, 100);
        let now = Instant::now();

        assert_eq!(throttle.admit("1.1.1.1", now), ALLOWED);
        assert_eq!(throttle.admit("1.1.1.1", now), ALLOWED);
        assert_eq!(throttle.admit("1.1.1.1", now), ALLOWED);
        assert_eq!(throttle.admit("1.1.1.1", now), Admission::Blocked);
        assert_eq!(throttle.admit("2.2.2.2", now), ALLOWED);
    }

    #[test]
    fn concurrent_attempts_cannot_exceed_the_client_limit() {
        let throttle = std::sync::Arc::new(throttle(5, 1_000));
        let now = Instant::now();

        let admitted: usize = (0..50)
            .map(|_| {
                let throttle = throttle.clone();
                std::thread::spawn(move || throttle.admit("1.1.1.1", now) != Admission::Blocked)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| usize::from(handle.join().unwrap()))
            .sum();

        assert_eq!(admitted, 5);
    }

    #[test]
    fn blocks_expire_with_the_window() {
        let throttle = throttle(1, 100);
        let now = Instant::now();

        attempt(&throttle, "1.1.1.1", 1, now);
        assert_eq!(
            throttle.admit("1.1.1.1", now + WINDOW - Duration::from_secs(1)),
            Admission::Blocked
        );
        assert_eq!(throttle.admit("1.1.1.1", now + WINDOW), ALLOWED);
    }

    #[test]
    fn a_success_clears_the_client_attempts() {
        let throttle = throttle(3, 100);
        let now = Instant::now();

        attempt(&throttle, "1.1.1.1", 3, now);
        throttle.record_success("1.1.1.1");

        assert_eq!(throttle.admit("1.1.1.1", now), ALLOWED);
    }

    #[test]
    fn the_global_limit_slows_everyone_down_without_locking_anyone_out() {
        let throttle = throttle(3, 5);
        let now = Instant::now();

        for client in 0..5 {
            attempt(&throttle, &format!("10.0.0.{client}"), 1, now);
        }

        assert_eq!(throttle.admit("192.168.1.1", now), SLOWED);
        assert_eq!(throttle.admit("192.168.1.1", now), SLOWED);
        assert_eq!(throttle.admit("192.168.1.2", now + WINDOW), ALLOWED);
    }

    #[test]
    fn successes_do_not_count_towards_the_global_limit() {
        let throttle = throttle(3, 2);
        let now = Instant::now();

        for _ in 0..10 {
            assert_eq!(throttle.admit("owner", now), ALLOWED);
            throttle.record_success("owner");
        }
        attempt(&throttle, "attacker", 2, now);
        assert_eq!(throttle.admit("owner", now), SLOWED);
    }

    #[test]
    fn expired_clients_are_forgotten() {
        let throttle = throttle(3, 100);
        let now = Instant::now();

        attempt(&throttle, "1.1.1.1", 1, now);
        attempt(&throttle, "2.2.2.2", 1, now + WINDOW);

        assert_eq!(throttle.lock().clients.len(), 1);
    }
}
