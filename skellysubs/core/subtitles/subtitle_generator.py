from openai.types.audio import TranscriptionVerbose

from skellysubs.core.subtitles.formatters.base_subtitle_formatter import FormattedSubtitles
from skellysubs.core.subtitles.formatters.md_subtitle_formatter import MDFormatter
from skellysubs.core.subtitles.formatters.srt_subtitle_formatter import SrtSubtitleFormatter
from skellysubs.core.subtitles.formatters.vtt_subtitle_formatter import VttSubtitleFormatter
from skellysubs.core.subtitles.subtitle_types import SubtitleFormats
from skellysubs.core.translation.models.translated_transcript import TranslatedTranscript


class SubtitleGenerator:
    """
    Main entry point for generating subtitles in multiple formats

    Takes a transcript and generates subtitle files in all supported formats
    (SRT, MD, etc). Handles the complexity of different subtitle formats and
    variants internally.

    This class implements the Facade pattern, providing a simplified interface
    to the subtitle generation subsystem. It coordinates multiple formatters
    to produce consistent output across different formats.

    Example:
        generator = SubtitleGenerator()
        formatted_subtitles = generator.generate_all_formats(transcript)

    Technical Details:
        - Implements the Facade design pattern
        - Manages multiple format-specific formatters internally
        - Ensures consistent output across different subtitle formats
        - Handles both translated and original transcript inputs
    """

    def __init__(self):
        self._formatters = {
            SubtitleFormats.SRT: SrtSubtitleFormatter(),
            SubtitleFormats.MD: MDFormatter(),
            SubtitleFormats.VTT: VttSubtitleFormatter()
        }

    def generate_all_formats(
            self,
            transcript: TranslatedTranscript | TranscriptionVerbose
    ) -> FormattedSubtitles:
        """
        Generate subtitles in all supported formats from a transcript

        Args:
            transcript: The transcript to convert into subtitles

        Returns:
            List of FormattedSubtitles, each containing the subtitle content
            in multiple formats (SRT, MD, etc) for a specific type
            (original, translated, romanized, etc)
        """
        formatted_subtitles = {}

        for format_type, formatter in self._formatters.items():
            formatted_subtitles[format_type.value] = formatter.format_transcript(transcript)

        return FormattedSubtitles(srt= formatted_subtitles[SubtitleFormats.SRT.value],
                                  vtt= formatted_subtitles[SubtitleFormats.VTT.value],
                                  md= formatted_subtitles[SubtitleFormats.MD.value])
