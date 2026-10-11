//! Per-invocation recording ownership; fingerprints only select replay queues.
use super::*;

impl State {
    pub(super) fn require_live_network(&mut self) -> Result<()> {
        if self.mode == Mode::Replay {
            return Err(self.fail("live_network_during_replay"));
        }
        Ok(())
    }
    pub(super) fn new(directory: &Path, mode: Mode, tape: Tape) -> Result<Self> {
        if tape.format != 1 || tape.records.len() > MAX_CALLS {
            return Err(error());
        }
        let mut replay: BTreeMap<String, VecDeque<Record>> = BTreeMap::new();
        for record in tape.records {
            if !record.complete
                || record.fingerprint.len() != 64
                || !record
                    .fingerprint
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                || (record.frames.is_empty() && record.cancelled_micros.is_none())
                || record.frames.len() > MAX_FRAMES
            {
                return Err(error());
            }
            if record
                .frames
                .windows(2)
                .any(|pair| pair[0].elapsed_micros > pair[1].elapsed_micros)
            {
                return Err(error());
            }
            if record.cancelled_micros.is_some_and(|time| {
                record
                    .frames
                    .last()
                    .is_some_and(|frame| frame.elapsed_micros > time)
            }) {
                return Err(error());
            }
            replay
                .entry(record.fingerprint.clone())
                .or_default()
                .push_back(record);
        }
        Ok(Self {
            directory: directory.into(),
            mode,
            started: false,
            used: false,
            failed: false,
            failure_reason: None,
            next_call: 0,
            bytes: 0,
            active: BTreeMap::new(),
            records: Vec::new(),
            replay,
            epoch: Instant::now(),
        })
    }
    pub(super) fn start(&mut self) -> Result<()> {
        if self.failed {
            return Err(self.failure());
        }
        if self.used || self.failed || !self.active.is_empty() {
            return Err(error());
        }
        self.started = true;
        self.used = true;
        Ok(())
    }
    pub(super) fn begin(
        &mut self,
        kind: Kind,
        fingerprint: String,
    ) -> Result<(bool, Option<Record>, u64)> {
        let started = Instant::now();
        if self.failed {
            return Err(self.failure());
        }
        if self.mode == Mode::Replay && !self.started {
            return Err(self.fail("replay_not_started"));
        }
        let replay = if self.mode == Mode::Replay {
            match self
                .replay
                .get_mut(&fingerprint)
                .and_then(VecDeque::pop_front)
            {
                Some(record) => Some(record),
                None => {
                    return Err(self.fail("replay_request_not_found"));
                }
            }
        } else {
            None
        };
        let record_index =
            (self.started && self.mode == Mode::Capture).then_some(self.records.len());
        if self.started && self.mode == Mode::Capture {
            if self.records.len() >= MAX_CALLS {
                return Err(self.fail("capture_call_limit"));
            }
            self.records.push(Record {
                kind,
                fingerprint: fingerprint.clone(),
                start_micros: started
                    .duration_since(self.epoch)
                    .as_micros()
                    .min(u64::MAX as u128) as u64,
                frames: Vec::new(),
                complete: false,
                cancelled_micros: None,
            });
        }
        let id = self.next_call;
        self.next_call = self.next_call.checked_add(1).ok_or_else(error)?;
        self.active.insert(
            id,
            ActiveCall {
                started,
                record: record_index,
            },
        );
        Ok((self.started, replay, id))
    }
    pub(super) fn stop(&mut self) -> Result<Option<Tape>> {
        if self.failed {
            return Err(self.failure());
        }
        if !self.started
            || self.failed
            || !self.active.is_empty()
            || (self.mode == Mode::Replay && self.replay.values().any(|v| !v.is_empty()))
        {
            return Err(self.fail(if !self.started {
                "stop_before_start"
            } else if !self.active.is_empty() {
                "stop_with_active_calls"
            } else {
                "replay_records_remaining"
            }));
        }
        self.started = false;
        Ok((self.mode == Mode::Capture).then(|| Tape {
            format: 1,
            records: self.records.clone(),
        }))
    }

    pub(super) fn failure(&self) -> AppError {
        AppError::new(
            ErrorCode::Validation,
            format!(
                "Workflow capture failed: {}.",
                self.failure_reason.unwrap_or("unknown_capture_failure")
            ),
        )
    }
    pub(super) fn fail(&mut self, reason: &'static str) -> AppError {
        if !self.failed {
            self.failure_reason = Some(reason);
            self.failed = true;
            let diagnostic = serde_json::json!({
                "reason": reason,
                "mode": if self.mode == Mode::Capture { "capture" } else { "replay" },
                "elapsed_micros": self.epoch.elapsed().as_micros().min(u64::MAX as u128) as u64,
                "active_calls": self.active.len(), "recorded_calls": self.records.len(),
                "captured_bytes": self.bytes
            });
            // Authored reason and counts only; never inputs, credentials or responses.
            if std::fs::write(
                self.directory.join("native-capture-failure.json"),
                diagnostic.to_string(),
            )
            .is_err()
            {
                return AppError::new(
                    ErrorCode::Validation,
                    format!(
                        "Workflow capture failed: {reason}. Failure report could not be saved."
                    ),
                );
            }
        }
        self.failure()
    }
    pub(super) fn push(&mut self, id: u64, payload: Value) -> Result<()> {
        if self.failed {
            return Err(self.failure());
        }
        let active = self.active.get(&id).ok_or_else(error)?;
        let index = active.record.ok_or_else(error)?;
        let elapsed_micros = active.started.elapsed().as_micros().min(u64::MAX as u128) as u64;
        self.bytes = self
            .bytes
            .saturating_add(serde_json::to_vec(&payload)?.len() + 64);
        if self.bytes > LIMIT {
            return Err(self.fail("capture_byte_limit"));
        }
        if self.records[index].frames.len() >= MAX_FRAMES {
            return Err(self.fail("capture_frame_limit"));
        }
        self.records[index].frames.push(Frame {
            elapsed_micros,
            payload,
        });
        Ok(())
    }
    pub(super) fn finish(&mut self, id: u64) -> Result<()> {
        let active = self.active.remove(&id).ok_or_else(error)?;
        if self.failed {
            return Err(self.failure());
        }
        if let Some(index) = active.record {
            self.records[index].complete = true;
        }
        Ok(())
    }
    pub(super) fn cancel(&mut self, id: u64, recording: bool, expected_cancellation: bool) {
        let active = self.active.remove(&id);
        if recording && !self.failed {
            if let Some(active) = active
                && let Some(index) = active.record
            {
                self.records[index].cancelled_micros =
                    Some(active.started.elapsed().as_micros().min(u64::MAX as u128) as u64);
                self.records[index].complete = true;
            } else {
                self.fail("capture_call_missing_on_cancel");
            }
        } else if self.mode == Mode::Replay && !expected_cancellation {
            self.fail("unexpected_replay_cancellation");
        }
    }
}
