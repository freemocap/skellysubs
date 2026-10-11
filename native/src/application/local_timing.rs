//! Opt-in development measurements; no content, provider data or workspace writes.
use std::{
    panic::Location,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

#[derive(serde::Serialize)]
pub(super) struct Sample {
    file: &'static str,
    line: u32,
    start_micros: u64,
    wait_micros: u64,
    held_micros: u64,
}
struct Recording {
    started: Instant,
    samples: Vec<Sample>,
    overflowed: bool,
}
#[derive(Default)]
pub(super) struct Measurements {
    enabled: AtomicBool,
    recording: Mutex<Option<Recording>>,
}
pub(super) struct Pending<'a> {
    measurements: &'a Measurements,
    site: &'static Location<'static>,
    started: Instant,
    acquired: Instant,
}
impl Measurements {
    pub(super) fn start(&self) {
        *self.recording.lock().unwrap_or_else(|e| e.into_inner()) = Some(Recording {
            started: Instant::now(),
            samples: Vec::new(),
            overflowed: false,
        });
        self.enabled.store(true, Ordering::Release);
    }
    pub(super) fn finish(&self) -> serde_json::Value {
        self.enabled.store(false, Ordering::Release);
        let record = self
            .recording
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        match record {
            Some(record) => {
                serde_json::json!({"samples":record.samples,"overflowed":record.overflowed})
            }
            None => serde_json::json!({"samples":[],"inactive":true}),
        }
    }
    pub(super) fn acquired(
        &self,
        site: &'static Location<'static>,
        started: Instant,
    ) -> Option<Pending<'_>> {
        if !self.enabled.load(Ordering::Acquire) {
            return None;
        }
        Some(Pending {
            measurements: self,
            site,
            started,
            acquired: Instant::now(),
        })
    }
}
impl Pending<'_> {
    pub(super) fn released(self, released: Instant) {
        let mut guard = self
            .measurements
            .recording
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let Some(record) = guard.as_mut() else { return };
        if self.started < record.started {
            return;
        }
        if record.samples.len() >= 100_000 {
            record.overflowed = true;
            return;
        }
        let micros =
            |duration: std::time::Duration| duration.as_micros().min(u64::MAX as u128) as u64;
        record.samples.push(Sample {
            file: self.site.file(),
            line: self.site.line(),
            start_micros: micros(self.started.duration_since(record.started)),
            wait_micros: micros(self.acquired.duration_since(self.started)),
            held_micros: micros(released.duration_since(self.acquired)),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measurements_are_explicit_bounded_and_ignore_work_started_before_capture() {
        let measurements = Measurements::default();
        assert!(
            measurements
                .acquired(Location::caller(), Instant::now())
                .is_none()
        );
        measurements.start();
        let old = measurements
            .acquired(Location::caller(), Instant::now())
            .unwrap();
        measurements.start();
        old.released(Instant::now());
        for _ in 0..100_001 {
            measurements
                .acquired(Location::caller(), Instant::now())
                .unwrap()
                .released(Instant::now());
        }
        let value = measurements.finish();
        assert_eq!(value["samples"].as_array().unwrap().len(), 100_000);
        assert_eq!(value["overflowed"], true);
        assert_eq!(measurements.finish()["inactive"], true);
    }
}
