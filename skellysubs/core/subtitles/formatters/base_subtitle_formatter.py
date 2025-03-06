from abc import ABC, abstractmethod
from typing import Sequence

from openai.types.audio import TranscriptionVerbose
from openai.types.audio.transcription_segment import TranscriptionSegment
from pydantic import BaseModel, field_validator

from skellysubs.core.subtitles.subtitle_types import SrtFormattedString, VttFormattedString, SsaFormattedString, \
    MdFormattedString, FormattedSubtitleStringsByType
from skellysubs.core.translation.models.translated_transcript import TranslatedTranscript
from skellysubs.core.translation.models.translated_transcript_segment import TranslatedTranscriptSegment


class SubtitleValidationError(Exception):
    """Raised when subtitle validation fails for timing or format constraints"""
    pass


class FormattedSubtitles(BaseModel):
    """
    Container for all supported subtitle format variants of a translation

    Attributes:
        type: The type of subtitle content (original, translated, etc)
        srt: SubRip format subtitles
        vtt: WebVTT format subtitles
        ssa: SubStation Alpha format subtitles
        md: Markdown format subtitles with aligned translations
    """
    srt: SrtFormattedString
    vtt: VttFormattedString
    ssa: SsaFormattedString
    md: MdFormattedString

    @field_validator('srt')
    def validate_srt_format(cls, v):
        if not v.strip():
            raise ValueError("SRT content cannot be empty")
        if not v.replace('\n', '').replace('\r', '').strip():
            raise ValueError("SRT content cannot be only whitespace")
        return v

    @field_validator('vtt')
    def validate_vtt_format(cls, v):
        if not v.startswith('WEBVTT'):
            raise ValueError("VTT content must start with WEBVTT header")
        return v

    class Config:
        frozen = True


class SubtitleFormatter(ABC):
    """
    Abstract base class for subtitle formatters

    Defines common validation and formatting operations for subtitle generation.
    Concrete implementations should handle specific subtitle format requirements.
    """

    def validate_segments(self, segments: Sequence[TranslatedTranscriptSegment | TranscriptionSegment]) -> None:
        """
        Validate subtitle timing and format constraints

        Args:
            segments: Sequence of translated segments to validate

        Raises:
            SubtitleValidationError: If any timing or format constraints are violated
        """
        if not segments:
            raise SubtitleValidationError("No segments provided")

        previous_end = 0

        for segment in segments:
            # Validate start/end logic
            if segment.end <= segment.start:
                raise SubtitleValidationError(
                    f"Invalid timing: end ({segment.end}) must be after start ({segment.start})"
                )

            # Check for overlaps and minimum gaps
            if segment.start < previous_end:
                raise SubtitleValidationError(
                    f"Subtitle overlap detected at {segment.start}s"
                )

            previous_end = segment.end

    @abstractmethod
    def format_transcript(
            self,
            transcript: TranslatedTranscript | TranscriptionVerbose
    ) -> FormattedSubtitleStringsByType:
        """
        Format translated transcript into subtitle format with variants

        Args:
            transcript: Either a translated transcript or raw transcription to format

        Returns:
            Dictionary mapping subtitle types to formatted subtitle strings
        """
        pass
