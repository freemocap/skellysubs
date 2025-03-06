from openai.types.audio import TranscriptionVerbose

from skellysubs.core.subtitles.formatters.base_subtitle_formatter import SubtitleFormatter
from skellysubs.core.subtitles.formatters.subtitle_time_formatter import SubtitleTimeFormatter
from skellysubs.core.subtitles.subtitle_types import (
    SubtitleTypes,
    FormattedSubtitleStringsByType
)
from skellysubs.core.translation.models.translated_transcript import TranslatedTranscript


class MDFormatter(SubtitleFormatter):
    """
    Markdown subtitle format handler

    Implements formatting for .md subtitle files, creating an aligned view of
    translations with timestamps that can be rendered as a readable document.
    """

    def format_transcript(
            self,
            transcript: TranslatedTranscript | TranscriptionVerbose
    ) -> FormattedSubtitleStringsByType:
        """Format a translated transcript into Markdown format with variants"""
        self.validate_segments(transcript.translated_segments)

        subtitle_types = {}
        language_config = transcript.translated_language

        # Base translated version
        base_md = ["# Transcript"]
        for segment in transcript.translated_segments:
            timestamp = SubtitleTimeFormatter.format_time_markdown(segment.start)
            base_md.append(
                f"\n[{timestamp}] {segment.translated_text.translated_text.strip()}"
            )
        subtitle_types[SubtitleTypes.translation_only] = "\n".join(base_md)

        # Romanized version if applicable
        if isinstance(transcript, TranslatedTranscript) and language_config.romanization_method:
            subtitle_types[SubtitleTypes.translation_with_romanization] = self._format_romanized(transcript)

        return subtitle_types

    @staticmethod
    def _format_romanized(translated_transcript: TranslatedTranscript) -> str:
        """
        Format transcript with both translated and romanized text in Markdown

        Args:
            translated_transcript: Transcript containing translations and romanization

        Returns:
            Markdown formatted string with both translated and romanized text
        """
        md_lines = ["# Transcript with Romanization"]

        for segment in translated_transcript.translated_segments:
            timestamp = SubtitleTimeFormatter.format_time_markdown(segment.start)
            translated_text = segment.translated_text.translated_text.strip()
            romanized_text = segment.translated_text.romanized_text.strip()

            md_lines.extend([
                f"\n[{timestamp}]",
                f"{translated_text}",
                f"_{romanized_text}_"  # italics for romanization
            ])

        return "\n".join(md_lines)
