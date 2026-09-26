use std::sync::Arc;

use parking_lot::{Condvar, Mutex};

pub(super) struct BatchBudget {
    state: Mutex<(usize, bool)>,
    changed: Condvar,
    max_bytes: usize,
}

impl BatchBudget {
    pub(super) fn new(max_bytes: usize) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new((0, false)),
            changed: Condvar::new(),
            max_bytes,
        })
    }

    pub(super) fn acquire(self: &Arc<Self>, bytes: usize) -> Option<Reservation> {
        let mut state = self.state.lock();
        // One oversized file can proceed only when no other payload is held.
        while !state.1 && state.0 > 0 && bytes > self.max_bytes.saturating_sub(state.0) {
            self.changed.wait(&mut state);
        }
        if state.1 {
            return None;
        }
        state.0 += bytes;
        Some(Reservation {
            budget: self.clone(),
            bytes,
        })
    }

    pub(super) fn stop(&self) {
        self.state.lock().1 = true;
        self.changed.notify_all();
    }
}

pub(super) struct Reservation {
    budget: Arc<BatchBudget>,
    bytes: usize,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.budget.state.lock().0 -= self.bytes;
        self.budget.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_payload_releases_waiter_and_stop_cancels_waiter() {
        let budget = BatchBudget::new(10);
        let first = budget.acquire(8).unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let waiting = budget.clone();
        let thread = std::thread::spawn(move || sender.send(waiting.acquire(8)).unwrap());
        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_millis(20))
                .is_err()
        );
        drop(first);
        let second = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
            .unwrap();
        thread.join().unwrap();
        assert_eq!(budget.state.lock().0, 8);
        budget.stop();
        assert!(budget.acquire(8).is_none());
        drop(second);
        assert_eq!(budget.state.lock().0, 0);
    }

    #[test]
    fn oversized_payload_runs_alone() {
        let budget = BatchBudget::new(10);
        let large = budget.acquire(20).unwrap();
        let waiting = budget.clone();
        let thread = std::thread::spawn(move || waiting.acquire(1));
        budget.stop();
        assert!(thread.join().unwrap().is_none());
        drop(large);
    }
}
