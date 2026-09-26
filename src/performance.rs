use std::time::Instant;

/// Emit one diagnostic per completed stage, including early returns.
pub(crate) struct StageTimer {
    stage: &'static str,
    started: Instant,
}

impl StageTimer {
    pub(crate) fn start(stage: &'static str) -> Self {
        Self {
            stage,
            started: Instant::now(),
        }
    }
}

impl Drop for StageTimer {
    fn drop(&mut self) {
        tracing::debug!(
            target: "ivygrep::performance",
            stage = self.stage,
            elapsed_ms = self.started.elapsed().as_secs_f64() * 1000.0,
            "stage completed"
        );
    }
}
