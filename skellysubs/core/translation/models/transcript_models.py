from abc import ABC, abstractmethod

from openai.types.audio import TranscriptionVerbose
from pydantic import BaseModel, field_validator, model_validator

from skellysubs.core.translation.language_configs.language_configs import LanguageConfig
from skellysubs.core.translation.models.text_models import TranslatedTextModel, OriginalLanguageTextModel, BaseTextModel
from skellysubs.core.translation.models.transcript_segment_models import TranslatedTranscriptSegment, \
    OriginalLanguageTranscriptSegment, BaseTranscriptSegment
from skellysubs.core.translation.models.translation_typehints import LanguageNameString


class BaseTranscript(BaseModel, ABC):
    text_model: BaseTextModel
    segments: list[BaseTranscriptSegment]
    duration: float


    @property
    @abstractmethod
    def text(self) -> str:
        pass

    @property
    @abstractmethod
    def language(self) -> LanguageNameString:
        pass

    @property
    @abstractmethod
    def romanized_text(self) -> str | None:
        return self.text_model.romanized_text


    @field_validator('duration', check_fields=False)
    @classmethod
    def validate_duration(cls, value: float, info):
        if value <= 0:
            raise ValueError(f"Duration must be greater than 0, received {value}")
        return round(value, 3)

    @field_validator('segments', check_fields=False)
    @classmethod
    def validate_segment_times(cls, value: list[TranslatedTranscriptSegment], info):
        if len(value) == 0:
            raise ValueError("Segments must have at least one segment")
        for segment in value:
            if segment.start < 0:
                raise ValueError(f"Segment start time must be greater than or equal to 0, received {segment.start}")
            if segment.end <= segment.start:
                raise ValueError(
                    f"Segment end time must be greater than start time, received {segment.end} and {segment.start}")
        return value

    @model_validator(mode='after')
    def validate_segments(self):
        if len(self.segments) == 0:
            raise ValueError("Segments must have at least one segment")
        for segment in self.segments:
            if len(segment.text) == 0:
                segment.text = "..."
                if segment.romanized_text:
                    segment.romanized_text = "..."
        return self


class OriginalLanguageTranscript(BaseTranscript):
    text_model: OriginalLanguageTextModel
    segments: list[OriginalLanguageTranscriptSegment]

    @property
    def text(self) -> str:
        return self.text_model.text

    @property
    def language(self) -> LanguageNameString:
        return self.text_model.language_name

    @property
    def romanized_text(self) -> str | None:
        return self.text_model.romanized_text

    @classmethod
    def from_openai_transcript(cls, transcript: TranscriptionVerbose):
        return cls( text_model=OriginalLanguageTextModel(text=transcript.text,
                                                        language_name=transcript.language),
                   duration=transcript.duration,
                   segments=[OriginalLanguageTranscriptSegment.from_openai_segment(segment=segment,
                                                                                   language_name=transcript.language)
                             for segment in transcript.segments]
                   )


class TranslatedTranscript(BaseModel):
    original_full_text: str
    translated_full_text: TranslatedTextModel
    original_language: LanguageNameString
    translated_language_config: LanguageConfig
    segments: list[TranslatedTranscriptSegment]

    @property
    def text(self):
        return self.translated_full_text.translated_text

    @property
    def language(self):
        return self.translated_language_config.language_name
