//! Bounded in-flight audio, committed with the native producer's settlement.
//! Payloads never enter native graph values, checkpoints or history segments.
use super::*;
use crate::speech::{alignment::SpeechAlignment, delivery, graph_audio, synthesis_graph};

pub(super) struct PendingAudio {
    pub identity: InvocationIdentity,
    pub request: synthesis_graph::Request,
    pub wav: Vec<u8>,
    pub alignment: Option<SpeechAlignment>,
}
impl PendingAudio {
    pub fn retain(&self, db: &Connection, values: &Values) -> Result<()> {
        let receipt: synthesis_graph::Receipt =
            serde_json::from_value(values["audio"]["receipt"].clone())?;
        let actual = graph_audio::reference(&self.identity, &self.wav)?;
        if serde_json::to_value(&receipt)? != serde_json::to_value(&actual)?
            || values["audio"]["source"] != serde_json::to_value(&self.request.source)?
        {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "Speech settlement differs from its producer payload.",
            ));
        }
        graph_audio::retain(
            db,
            &self.identity,
            &self.request,
            &self.wav,
            self.alignment.as_ref(),
        )?;
        Ok(())
    }
}
impl Runtime {
    pub fn authorize_speech(
        &self,
        db: &Connection,
        identity: &InvocationIdentity,
    ) -> Result<synthesis_graph::Request> {
        let work = self.authorize_work(db, identity)?;
        if work.operation != synthesis_graph::operation_contract()
            && work.operation != synthesis_graph::playback::lookup_operation()
        {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "Native producer is not a speech request.",
            ));
        }
        synthesis_graph::decode(&work.inputs).map_err(error)
    }

    pub fn stage_audio(
        &mut self,
        db: &Connection,
        identity: &InvocationIdentity,
        wav: Vec<u8>,
        alignment: Option<SpeechAlignment>,
    ) -> Result<synthesis_graph::Receipt> {
        let request = self.authorize_speech(db, identity)?;
        let receipt = graph_audio::reference(identity, &wav)?;
        if alignment
            .as_ref()
            .is_some_and(|a| !a.valid() || a.source_text != request.source.text)
        {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Speech alignment differs from its source.",
            ));
        }
        if self.audio.contains_key(&receipt.id)
            || self.audio.len() >= delivery::DELIVERY_ENTRIES
            || self.audio.values().map(|a| a.wav.len()).sum::<usize>() + wav.len()
                > delivery::DELIVERY_BYTES
        {
            return Err(AppError::new(
                ErrorCode::AdmissionHeld,
                "Native audio settlement buffer is full or already owns this producer.",
            ));
        }
        self.audio.insert(
            receipt.id.clone(),
            PendingAudio {
                identity: identity.clone(),
                request,
                wav,
                alignment,
            },
        );
        Ok(receipt)
    }

    pub fn lookup_audio(
        &mut self,
        db: &Connection,
        identity: &InvocationIdentity,
    ) -> Result<Option<synthesis_graph::Receipt>> {
        let request = self.authorize_speech(db, identity)?;
        if identity.operation != synthesis_graph::playback::lookup_operation() {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "Cache lookup requires its native lookup producer.",
            ));
        }
        let tx = db.unchecked_transaction()?;
        let cached = graph_audio::lookup(&tx, &request)?;
        tx.commit()?;
        let Some(cached) = cached else {
            return Ok(None);
        };
        self.audio_delivery.insert(delivery::ReadyAudio {
            operation_id: cached.receipt.id.clone(),
            attempt_id: cached.receipt.id.clone(),
            message_id: request.source.id,
            wav: cached.wav,
            alignment: cached.alignment,
        })?;
        Ok(Some(cached.receipt))
    }
}
