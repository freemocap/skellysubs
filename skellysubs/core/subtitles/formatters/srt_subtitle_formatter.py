from skellysubs.core.subtitles.formatters.base_subtitle_formatter import SubtitleFormatter
from openai.types.audio import TranscriptionVerbose

from skellysubs.core.subtitles.formatters.subtitle_time_formatter import SubtitleTimeFormatter
from skellysubs.core.subtitles.subtitle_types import (
    SubtitleTypes,
    FormattedSubtitleStringsByType
)
from skellysubs.core.translation.models.translated_transcript import TranslatedTranscript


class SrtSubtitleFormatter(SubtitleFormatter):
    """
    SubRip (SRT) subtitle format handler

    Implements formatting for .srt subtitle files, handling both basic transcripts
    and translated content with optional romanization.
    """

    def format_transcript(
            self,
            transcript: TranslatedTranscript | TranscriptionVerbose
    ) -> FormattedSubtitleStringsByType:

        """Format a translated transcript into SRT format with variants"""
        self.validate_segments(transcript.translated_segments)

        subtitle_types = {}
        language_config = transcript.translated_language

        # Base translated version
        base_srt = []
        for index, segment in enumerate(transcript.translated_segments, 1):
            start_time = SubtitleTimeFormatter.format_time_srt(segment.start)
            end_time = SubtitleTimeFormatter.format_time_srt(segment.end)
            base_srt.append(
                f"{index}\n{start_time} --> {end_time}\n"
                f"{segment.translated_text.translated_text.strip()}"
            )
        subtitle_types[SubtitleTypes.translation_only] = "\n\n".join(base_srt)

        # Romanized version if applicable
        if isinstance(transcript, TranslatedTranscript) and language_config.romanization_method:
            subtitle_types[SubtitleTypes.translation_with_romanization] = self._format_romanized(transcript)

        return subtitle_types

    @staticmethod
    def _format_romanized(translated_transcript:TranslatedTranscript) -> str:
        """
        Format transcript with both translated and romanized text

        Args:
            translated_transcript: Transcript containing translations and romanization

        Returns:
            SRT formatted string with both translated and romanized text
        """
        romanized_srt = []
        for index, segment in enumerate(translated_transcript.translated_segments, 1):
            start = segment.start + 0.01 if segment.start == 0 else segment.start
            romanized_text = segment.translated_text.romanized_text.strip()
            translated_text = segment.translated_text.translated_text.strip()

            romanized_srt.append(
                f"{index}\n"
                f"{SubtitleTimeFormatter.format_time_srt(start)} --> "
                f"{SubtitleTimeFormatter.format_time_srt(segment.end)}\n"
                f"{translated_text}\n{romanized_text}"
            )
        return "\n\n".join(romanized_srt)