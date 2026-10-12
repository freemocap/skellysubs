use super::*;
impl Store {
    pub(crate) fn speech_stream_execution(&self, operation: &str) -> Result<Option<String>> {
        self.graph_runtime
            .speech_stream_execution(&self.connection, operation)
    }
    pub fn speech_audio(&self, operation: &str) -> Result<SpeechAudioState> {
        self.graph_runtime.speech_audio(&self.connection, operation)
    }
}
