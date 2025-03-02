from openai.types.audio import TranscriptionVerbose
from pydantic import BaseModel


class WhisperWordTimestamp(BaseModel):
    start: float
    end: float
    word: str
    index_in_segment: int
    index_in_transcript: int
    probability: float | None = None


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

    @classmethod
    def from_verbose_transcript(cls, verbose_transcript: TranscriptionVerbose):
        segments = []
        transcript_word_index = -1
        for segment_index, segment in enumerate(verbose_transcript.segments):
            words_in_this_segment = []
            segment_word_index = 0
            while verbose_transcript.words[0].start >= segment.start and verbose_transcript.words[0].end <= segment.end:
                word = verbose_transcript.words.pop(0)
                transcript_word_index += 1
                segment_word_index += 1
                words_in_this_segment.append(WhisperWordTimestamp(**word.model_dump(),
                                                                  index_in_segment=segment_word_index,
                                                                  index_in_transcript=transcript_word_index))

                if not verbose_transcript.words:
                    break
            whisper_segment = WhisperTranscriptSegment(
                id=segment_index,
                seek=segment.seek,
                start=segment.start,
                end=segment.end,
                text=segment.text,
                tokens=segment.tokens,
                temperature=segment.temperature,
                avg_logprob=segment.avg_logprob,
                compression_ratio=segment.compression_ratio,
                no_speech_prob=segment.no_speech_prob,
                words=words_in_this_segment if words_in_this_segment else None
            )
            segments.append(whisper_segment)
        return cls(
            text=verbose_transcript.text,
            segments=segments,
            language=verbose_transcript.language
        )
