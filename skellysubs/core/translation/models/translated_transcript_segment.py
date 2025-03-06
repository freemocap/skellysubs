from pydantic import BaseModel, Field, field_validator

from skellysubs.core.translation.models.translated_text import TranslatedText


class TranslatedTranscriptSegment(BaseModel):
    translated_text: TranslatedText = Field(
        description="The translated text in the target language, with romanization if applicable")
    original_segment_text: str = Field(description="The original text of the segment in its original language")
    start: float = Field(
        description="The start time of the period in the recording when the segment was spoken in seconds since the start of the recording. Should match the end time of the previous segment or the start time of the recording for the first segment.")
    end: float = Field(
        description="The end time of the segment in the recording when the segment was spoken in seconds since the start of the recording. Should match the start time of the next segment or the end time of the recording for the last segment.")
    duration: float = Field(description="The duration of the segment in seconds")

    @property
    def text(self) -> str:
        """
        Helper so we can use the same api as the TranslationVerbose class
        """
        return self.translated_text.translated_text

    @field_validator('start')
    @classmethod
    def validate_start_time(cls, value: float, info):
        end = info.data.get('end')
        if value < 0:
            raise ValueError(f"Start time must be greater than or equal to 0, received {value}")
        if end is not None and value >= end:
            raise ValueError(f"Start time must be less than end time, received {value} and {end}")
        return round(value, 3)

    @field_validator('end')
    @classmethod
    def validate_end_time(cls, value: float, info):
        start = info.data.get('start')
        if start is not None and value <= start:
            raise ValueError(f"End time must be greater than start time, received {value} and {start}")
        return round(value, 3)

    @field_validator('duration')
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