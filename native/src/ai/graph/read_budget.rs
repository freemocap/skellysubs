use super::{model::fault, *};

/// Aggregate native row reads per public owner operation. Repeated reads count
/// again. Bytes measure encoded row envelopes, not decoded heap size or RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordReadLimits {
    pub records: usize,
    pub bytes: usize,
}

/// One ledger spans decisions, native mutation and reservation reads. It wraps
/// the store, so a new typed reader cannot silently reset the operation's budget.
pub(super) struct BudgetedStore<'a, S> {
    store: &'a mut S,
    remaining: RecordReadLimits,
}

impl<'a, S> BudgetedStore<'a, S> {
    pub fn new(store: &'a mut S, limits: RecordReadLimits) -> Self {
        Self {
            store,
            remaining: limits,
        }
    }
}

impl<S: RecordStore> RecordStore for BudgetedStore<'_, S> {
    fn record_count(&mut self, expected: &Stamp) -> Result<usize> {
        self.store.record_count(expected)
    }

    fn read_record(
        &mut self,
        expected: &Stamp,
        key: &RecordKey,
        max_bytes: usize,
    ) -> Result<Vec<u8>> {
        if self.remaining.records == 0 {
            return Err(fault(CoreFaultCode::RecordReadCountLimit, "records"));
        }
        if self.remaining.bytes == 0 {
            return Err(fault(CoreFaultCode::RecordReadByteLimit, "records"));
        }
        let cap = max_bytes.min(self.remaining.bytes);
        let byte_fault = if cap < max_bytes {
            CoreFaultCode::RecordReadByteLimit
        } else {
            CoreFaultCode::RecordByteLimit
        };
        self.remaining.records -= 1;
        // Pass the remaining allowance BEFORE the adapter allocates the body.
        let bytes = self
            .store
            .read_record(expected, key, cap)
            .map_err(|error| {
                if cap < max_bytes && error.code == "record_byte_limit" {
                    fault(byte_fault, "records")
                } else {
                    error
                }
            })?;
        if bytes.len() > cap {
            return Err(fault(byte_fault, "records"));
        }
        self.remaining.bytes -= bytes.len();
        Ok(bytes)
    }
}

impl<S: HistoryStore> HistoryStore for BudgetedStore<'_, S> {
    fn read_archive(&mut self, expected: &Stamp, max_bytes: usize) -> Result<Vec<u8>> {
        self.store.read_archive(expected, max_bytes)
    }
}

impl<S: CommitStore> CommitStore for BudgetedStore<'_, S> {
    fn commit(&mut self, request: CommitRequest<'_>) -> std::result::Result<(), CommitFailure> {
        self.store.commit(request)
    }
}
