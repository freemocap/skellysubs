from pydantic import BaseModel


class WhisperWordTimestamp(BaseModel):
    start: float
    end: float
    word: str
    probability: float


class WhisperTranscriptSegment(BaseModel):
    id: int
    seek: float
    start: float
    end: float
    text: str
    tokens: list[int]
    temperature: float
    avg_logprob: float
    compression_ratio: float
    no_speech_prob: float
    words: list[WhisperWordTimestamp] | None = None


class WhisperTranscriptionResult(BaseModel):
    text: str
    segments: list[WhisperTranscriptSegment]
    language: str

