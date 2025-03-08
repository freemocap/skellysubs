from openai.types.audio import TranscriptionVerbose

from skellysubs.core.subtitles.formatters.base_subtitle_formatter import SubtitleFormatter
from skellysubs.core.subtitles.formatters.subtitle_time_formatter import SubtitleTimeFormatter
from skellysubs.core.subtitles.subtitle_types import (
    SubtitleVariant,
    FormattedSubtitleStringsByVariant
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
    ) -> FormattedSubtitleStringsByVariant:
        """Format a translated transcript into Markdown format with variants"""
        self.validate_segments(transcript.segments)

        subtitle_types = {}


        # Base translated version
        base_md = ["# Transcript"]
        if isinstance(transcript, TranslatedTranscript):
            base_md.append(f"## Original Language: {transcript.original_language}")
            base_md.append(f"## Translated Language: {transcript.translated_language.language_name}")
            if  transcript.translated_language.romanization_method and transcript.translated_language.romanization_method.lower() != "none":
                base_md.append(f"## Romanization Method: {transcript.translated_language.romanization_method}")

        elif isinstance(transcript, TranscriptionVerbose):
            base_md.append(f"## Spoken Language: {transcript.language}")
        else:
            raise ValueError("Invalid transcript type!")

        for segment in transcript.segments:
            timestamp = SubtitleTimeFormatter.format_time_markdown(segment.start)
            base_md.append(
                f"\n[{timestamp}] {segment.text.strip()}"
            )

        if isinstance(transcript, TranslatedTranscript):
            subtitle_types[SubtitleVariant.translation_only] = "\n".join(base_md)
            if  transcript.translated_language.romanization_method and transcript.translated_language.romanization_method.lower() != "none":
                subtitle_types[SubtitleVariant.translation_with_romanization] = self._format_romanized(transcript)
            subtitle_types[SubtitleVariant.multi_language] = self._format_multi_language(transcript)
        if isinstance(transcript, TranscriptionVerbose):
            subtitle_types[SubtitleVariant.original_spoken] = "\n".join(base_md)


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

        for segment in translated_transcript.segments:
            timestamp = SubtitleTimeFormatter.format_time_markdown(segment.start)
            translated_text = segment.translated_text.translated_text.strip()
            romanized_text = segment.translated_text.romanized_text.strip()

            md_lines.extend([
                f"\n[{timestamp}]",
                f"{translated_text}",
                f"_{romanized_text}_"  # italics for romanization
            ])

        return "\n".join(md_lines)

    @staticmethod
    def _format_multi_language(translated_transcript: TranslatedTranscript) -> str:
        """
        Format transcript with both translated and original text in Markdown

        Args:
            translated_transcript: Transcript containing translations and original text

        Returns:
            Markdown formatted string with both translated and original text
        """
        md_lines = ["# Transcript with Original Language"]

        for segment_number, segment in enumerate(translated_transcript.segments):
            md_lines.append(f"\n> Segment#{segment_number} of {len(translated_transcript.segments)} " + SubtitleTimeFormatter.format_time_markdown(segment.start))
            md_lines.append(f"\n> {segment.original_segment_text.strip()}")
            md_lines.append(f"\n> {segment.translated_text.translated_text.strip()}")
            if translated_transcript.translated_language.romanization_method and translated_transcript.translated_language.romanization_method.lower() != "none":
                md_lines.append("\n> "+segment.translated_text.romanized_text.strip())

        return "\n".join(md_lines)