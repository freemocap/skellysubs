from pydantic import BaseModel, Field

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