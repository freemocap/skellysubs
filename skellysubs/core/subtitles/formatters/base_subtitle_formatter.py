from abc import ABC, abstractmethod

from openai.types.audio import TranscriptionVerbose
from pydantic import BaseModel

from skellysubs.core.subtitles.subtitle_types import SrtFormattedString, VttFormattedString, MdFormattedString, \
    FormattedSubtitleStringsByVariant, SubtitleFormattedString, SubtitleVariant
from skellysubs.core.translation.models.translated_transcript import TranslatedTranscript


class SubtitleValidationError(Exception):
    """Raised when subtitle validation fails for timing or format constraints"""
    pass


class FormattedSubtitles(BaseModel):
    """
    Container for all supported subtitle format variants of a translation

    Attributes:
        srt: SubRip format subtitles
        # vtt: WebVTT format subtitles
        # ssa: SubStation Alpha format subtitles
        md: Markdown format transcription text and segments
    """
    srt: dict[SubtitleVariant, SrtFormattedString]
    vtt: dict[SubtitleVariant, VttFormattedString]
    # ssa: dict[SubtitleVariant, SsaFormattedString]
    # ass: dict[SubtitleVariant, AssFormattedString]
    # ttml: dict[SubtitleVariant, TtmlFormattedString]
    # scc : dict[SubtitleVariant, SccFormattedString]
    md: dict[SubtitleVariant, MdFormattedString]



    class Config:
        frozen = True


class SubtitleFormatter(ABC):
    """
    Abstract base class for subtitle formatters

    Defines common validation and formatting operations for subtitle generation.
    Concrete implementations should handle specific subtitle format requirements.
    """



    @abstractmethod
    def format_transcript(
            self,
            transcript: TranslatedTranscript | TranscriptionVerbose
    ) -> FormattedSubtitleStringsByVariant:
        """
        Format translated transcript into subtitle format with variants

        Args:
            transcript: Either a translated transcript or raw transcription to format

        Returns:
            Dictionary mapping subtitle types to formatted subtitle strings
        """
        pass

    @staticmethod
    @abstractmethod
    def _format_romanized(translated_transcript: TranslatedTranscript) -> SubtitleFormattedString:
        """
        Format subtitles with both translated and romanized text

        Args:
            translated_transcript: Transcript containing translations and romanization

        Returns:
            Subtitle formatted string with both translated and romanized text
        """
        pass

    @staticmethod
    @abstractmethod
    def _format_multi_language(translated_transcript: TranslatedTranscript) -> SubtitleFormattedString:
        """
        Format subtitles with both original and translated text (and romanization if applicable)
        """
        pass