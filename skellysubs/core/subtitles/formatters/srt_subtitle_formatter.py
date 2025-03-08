from openai.types.audio import TranscriptionVerbose

from skellysubs.core.subtitles.formatters.base_subtitle_formatter import SubtitleFormatter
from skellysubs.core.subtitles.formatters.subtitle_time_formatter import SubtitleTimeFormatter
from skellysubs.core.subtitles.subtitle_types import (
    SubtitleVariant,
    FormattedSubtitleStringsByVariant
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
    ) -> FormattedSubtitleStringsByVariant:
        """Format a translated transcript into SRT format with variants"""
        subtitle_types = {}

        if isinstance(transcript, TranscriptionVerbose):
            subtitle_types[SubtitleVariant.original_spoken] = self._format_transcription(transcript)
            return subtitle_types

        if isinstance(transcript, TranslatedTranscript):
            subtitle_types[SubtitleVariant.translation_only] = self._format_translation(transcript)

            if self._has_romanization(transcript):
                subtitle_types[SubtitleVariant.translation_with_romanization] = self._format_romanized(transcript)

            subtitle_types[SubtitleVariant.multi_language] = self._format_multi_language(transcript)
            return subtitle_types

        raise ValueError("Invalid transcript type!")


    @staticmethod
    def _format_segment(index: int, start_time: float, end_time: float, text: str) -> str:
        """Format a single SRT subtitle segment"""
        return (
            f"{index}\n"
            f"{SubtitleTimeFormatter.format_time_srt(start_time)} --> "
            f"{SubtitleTimeFormatter.format_time_srt(end_time)}\n"
            f"{text}"
        )

    @staticmethod
    def _format_transcription(transcript: TranscriptionVerbose) -> str:
        """Format a basic transcription into SRT"""
        srt_segments = []

        for index, segment in enumerate(transcript.segments, 1):
            srt_segments.append(
                SrtSubtitleFormatter._format_segment(
                    index=index,
                    start_time=segment.start,
                    end_time=segment.end,
                    text=segment.text.strip()
                )
            )

        return "\n\n".join(srt_segments)

    @staticmethod
    def _format_translation(transcript: TranslatedTranscript) -> str:
        """Format a translation into SRT"""
        srt_segments = []

        for index, segment in enumerate(transcript.segments, 1):
            srt_segments.append(
                SrtSubtitleFormatter._format_segment(
                    index=index,
                    start_time=segment.start,
                    end_time=segment.end,
                    text=segment.translated_text.translated_text.strip()
                )
            )

        return "\n\n".join(srt_segments)

    @staticmethod
    def _format_romanized(translated_transcript: TranslatedTranscript) -> str:
        """Format transcript with both translated and romanized text in SRT"""
        srt_segments = []

        for index, segment in enumerate(translated_transcript.segments, 1):
            # Adjust start time slightly if it's at 0 to prevent timing issues
            start = segment.start + 0.01 if segment.start == 0 else segment.start

            text = (
                f"{segment.translated_text.translated_text.strip()}\n"
                f"{segment.translated_text.romanized_text.strip()}"
            )

            srt_segments.append(
                SrtSubtitleFormatter._format_segment(
                    index=index,
                    start_time=start,
                    end_time=segment.end,
                    text=text
                )
            )

        return "\n\n".join(srt_segments)

    @staticmethod
    def _format_multi_language(translated_transcript: TranslatedTranscript) -> str:
        """Format transcript with original, translated, and romanized text in SRT"""
        srt_segments = []

        for index, segment in enumerate(translated_transcript.segments, 1):
            text_parts = [
                segment.original_segment_text.strip(),
                segment.translated_text.translated_text.strip()
            ]

            if SrtSubtitleFormatter._has_romanization(translated_transcript):
                text_parts.append(segment.translated_text.romanized_text.strip())

            srt_segments.append(
                SrtSubtitleFormatter._format_segment(
                    index=index,
                    start_time=segment.start,
                    end_time=segment.end,
                    text="\n".join(text_parts)
                )
            )

        return "\n\n".join(srt_segments)