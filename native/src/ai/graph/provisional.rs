//! Protected provisional source text, never a graph output or diagnostic DTO.
use super::{model::fault, *};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy)]
pub struct ProvisionalLimits {
    /// Maximum UTF-8 bytes in one cumulative replacement, not heap usage.
    pub bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisionalCapture {
    /// Callback-lifetime identity; distinct from a future UI subscription session.
    pub session: String,
    /// Accepted text replacements only. A latched failure does not invent text.
    pub sequence: u64,
    pub text: String,
    pub failure: Option<Fault>,
}

pub(super) fn present<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> std::result::Result<Option<ProvisionalCapture>, D::Error> {
    ProvisionalCapture::deserialize(decoder).map(Some)
}

pub(super) struct Capture {
    limits: ProvisionalLimits,
    session: String,
    pub value: Option<ProvisionalCapture>,
}

impl Capture {
    pub fn new(limits: ProvisionalLimits) -> Self {
        Self {
            limits,
            session: uuid::Uuid::new_v4().to_string(),
            value: None,
        }
    }

    pub fn replace(&mut self, text: &str) -> Result<()> {
        let current = self.value.get_or_insert_with(|| ProvisionalCapture {
            session: self.session.clone(),
            sequence: 0,
            text: String::new(),
            failure: None,
        });
        if let Some(error) = &current.failure {
            return Err(error.clone());
        }
        if current.sequence > 0 && current.text == text {
            return Ok(());
        }
        let checked = if text.len() > self.limits.bytes {
            Err(fault(CoreFaultCode::ProvisionalLimit, "provisional"))
        } else {
            current
                .sequence
                .checked_add(1)
                .ok_or_else(|| fault(CoreFaultCode::ProvisionalSequence, "provisional"))
        };
        match checked {
            Ok(sequence) => {
                current.sequence = sequence;
                current.text = text.into();
                Ok(())
            }
            Err(error) => {
                current.failure = Some(error.clone());
                Err(error)
            }
        }
    }
}

/// A watermark acknowledges the exact replacement, not a string prefix.
pub(super) fn validate_next(
    previous: Option<&ProvisionalCapture>,
    next: Option<&ProvisionalCapture>,
) -> Result<()> {
    let invalid = || fault(CoreFaultCode::ProvisionalConflict, "provisional");
    if let Some(next) = next
        && (uuid::Uuid::parse_str(&next.session).is_err()
            || (next.sequence == 0 && (!next.text.is_empty() || next.failure.is_none())))
    {
        return Err(invalid());
    }
    if let Some(previous) = previous {
        let next = next.ok_or_else(invalid)?;
        if previous.session != next.session
            || next.sequence < previous.sequence
            || (next.sequence == previous.sequence && next.text != previous.text)
            || (previous.failure.is_some() && previous != next)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacements_are_utf8_bounded_correctable_and_never_wrap() {
        let mut capture = Capture::new(ProvisionalLimits { bytes: 4 });
        capture.replace("海").unwrap();
        capture.replace("海").unwrap();
        assert_eq!(capture.value.as_ref().unwrap().sequence, 1);
        capture.replace("é").unwrap();
        assert_eq!(capture.value.as_ref().unwrap().sequence, 2);
        assert!(capture.replace("海海").is_err());
        assert_eq!(capture.value.as_ref().unwrap().text, "é");
        assert!(capture.replace("a").is_err(), "failure is latched");

        let mut capture = Capture::new(ProvisionalLimits { bytes: 4 });
        capture.replace("x").unwrap();
        capture.value.as_mut().unwrap().sequence = u64::MAX;
        assert_eq!(
            capture.replace("y").unwrap_err().code,
            "provisional_sequence"
        );
        assert_eq!(capture.value.as_ref().unwrap().text, "x");
        assert_eq!(capture.value.as_ref().unwrap().sequence, u64::MAX);
    }
}
