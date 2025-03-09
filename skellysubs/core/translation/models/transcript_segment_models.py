from abc import ABC, abstractmethod

from openai.types.audio.transcription_segment import TranscriptionSegment
from pydantic import BaseModel, Field, field_validator

from skellysubs.core.translation.models.text_models import TranslatedTextModel, OriginalLanguageTextModel
from skellysubs.core.translation.models.translation_typehints import LanguageNameString, LanguageISO6391CodeString


class BaseTranscriptSegment(BaseModel, ABC):
    start: float = Field(
        description="The start time of the period in the recording when the segment was spoken in seconds since the start of the recording. Should match the end time of the previous segment or the start time of the recording for the first segment.")
    end: float = Field(
        description="The end time of the segment in the recording when the segment was spoken in seconds since the start of the recording. Should match the start time of the next segment or the end time of the recording for the last segment.")
    duration: float = Field(description="The duration of the segment in seconds")


    @property
    @abstractmethod
    def text(self) -> str:
        """
        Helper to retrieve the primary text of the segment
        """
        pass


    @text.setter
    @abstractmethod
    def text(self, value: str):
        """
        Helper to set the primary text of the segment
        """
        pass

    @property
    @abstractmethod
    def romanized_text(self) -> str | None:
        """
        Helper to retrieve the romanized text of the segment (or None if not applicable)
        """
        pass

    @romanized_text.setter
    @abstractmethod
    def romanized_text(self, value: str):
        """
        Helper to set the romanized text of the segment
        """
        pass


    @field_validator('start', mode='before', check_fields=False)
    @classmethod
    def validate_start_time(cls, value: float, info):
        end = info.data.get('end')
        if value < 0:
            raise ValueError(f"Start time must be greater than or equal to 0, received {value}")
        if end is not None and value >= end:
            raise ValueError(f"Start time must be less than end time, received {value} and {end}")
        return round(value, 3)

    @field_validator('end', mode='before', check_fields=False)
    @classmethod
    def validate_end_time(cls, value: float, info):
        start = info.data.get('start')
        if start is not None and value <= start:
            raise ValueError(f"End time must be greater than start time, received {value} and {start}")
        return round(value, 3)

    @field_validator('duration', mode='before', check_fields=False)
    @classmethod
    def validate_duration(cls, value: float, info):
        if value <= 0:
            raise ValueError(f"Duration must be greater than 0, received {value}")

        start = info.data.get('start')
        end = info.data.get('end')

        if start is not None and end is not None:
            if abs(end - start - value) > 0.001:
                raise ValueError(f"Duration must match start and end times, received {value}, {start}, and {end}")

        return round(value, 3)


class OriginalLanguageTranscriptSegment(BaseTranscriptSegment):
    original_segment_text: OriginalLanguageTextModel = Field(
        description="The original text of the segment in its original language")

    @classmethod
    def from_openai_segment(cls, segment: TranscriptionSegment,
                            language_name: LanguageNameString):
        return cls(start=segment.start,
                   end=segment.end,
                   duration=segment.end - segment.start,
                   original_segment_text=OriginalLanguageTextModel(text=segment.text,
                                                                   language_name=language_name)
                   )

    @property
    def text(self) -> str:
        """
        Helper so we can use the same api as the TranslationVerbose class
        """
        return self.original_segment_text.text

    @text.setter
    def text(self, value: str):
        """
        Helper so we can use the same api as the TranslationVerbose class
        """
        self.original_segment_text.text = value

    @property
    def romanized_text(self) -> str | None:
        """
        Helper to retrieve the romanized text of the segment (or None if not applicable)
        """
        rt = self.original_segment_text.romanized_text
        if not rt or rt.lower() == "none":
            return None

    @romanized_text.setter
    def romanized_text(self, value: str):
        """
        Helper to set the romanized text of the segment
        """
        self.original_segment_text.romanized_text = value


class TranslatedTranscriptSegment(BaseTranscriptSegment):
    translated_text: TranslatedTextModel = Field(
        description="The translated text in the target language, with romanization if applicable")
    original_segment_text: OriginalLanguageTextModel = Field(
        description="The original text of the segment in its original language")

    @property
    def text(self) -> str:
        """
        Helper so we can use the same api as the TranslationVerbose class
        """
        return self.translated_text.translated_text

    @text.setter
    def text(self, value: str):
        """
        Helper so we can use the same api as the TranslationVerbose class
        """
        self.translated_text.translated_text = value

    @property
    def romanized_text(self) -> str | None:
        """
        Helper to retrieve the romanized text of the segment (or None if not applicable)
        """
        rt = self.translated_text.romanized_text
        if not rt or rt.lower() == "none":
            return None

    @romanized_text.setter
    def romanized_text(self, value: str):
        """
        Helper to set the romanized text of the segment
        """
        self.translated_text.romanized_text = value
