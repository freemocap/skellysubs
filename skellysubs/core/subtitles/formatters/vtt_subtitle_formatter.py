from pydantic import BaseModel

from typing import Optional
from openai.types.audio import TranscriptionVerbose

from skellysubs.core.subtitles.formatters.base_subtitle_formatter import SubtitleFormatter
from skellysubs.core.subtitles.formatters.subtitle_time_formatter import SubtitleTimeFormatter
from skellysubs.core.subtitles.subtitle_types import SubtitleVariant, FormattedSubtitleStringsByVariant
from skellysubs.core.translation.models.translated_transcript import TranslatedTranscript


class VttConfiguration(BaseModel):
    """
    Configuration for WebVTT subtitle generation with advanced features

    WebVTT supports features not available in SRT:
    - Styling directives
    - Positioning information
    - Vertical text layouts
    - Regional formatting
    - Chapter markers
    - Metadata tracks

    Reference: https://www.w3.org/TR/webvtt1/
    """
    vertical_text: str | None = None  # 'lr' or 'rl' for vertical text direction
    line_position: int | str | None = None  # Integer percentage or 'auto'
    text_align: str = "center"  # start|center|end|left|right
    region_settings: str | None = None  # WebVTT region settings string
    add_webvtt_header: bool = True  # Whether to include WEBVTT header
    description: str | None = "SkellySubs Generated Captions"  # Header note/description
    enable_positioning: bool = False  # Whether to use advanced positioning
    default_position: str = "line:0"  # Default cue positioning
    wrap_limit: int | None = None  # Maximum characters per line (None = auto)

    class Config:
        frozen = True  # we can alter these config values on the clientside



class VttSubtitleFormatter(SubtitleFormatter):
    """
    Enhanced WebVTT formatter with advanced formatting options

    Supports WebVTT 1.0 features including:
    - Vertical text layouts
    - Cue positioning
    - Regional formatting
    - Styling metadata
    - Chapter markers
    """

    def __init__(self, config: Optional[VttConfiguration] = None):
        self.config = config or VttConfiguration()

    def _build_vtt_header(self) -> str:
        """Construct the WebVTT header with configuration options"""
        header = []

        if self.config.add_webvtt_header:
            header.append("WEBVTT")

            if self.config.description:
                header.append(f"NOTE {self.config.description}")

            if self.config.region_settings:
                header.append(f"REGION {self.config.region_settings}")

            if self.config.vertical_text:
                header.append(f"STYLE::cue {{ vertical: {self.config.vertical_text} }}")

        return "\n".join(header) + "\n\n" if header else ""

    def _format_segment(self, index: int, start_time: float, end_time: float, text: str) -> str:
        """Format a WebVTT segment with configurable positioning"""
        timecode = (
            f"{SubtitleTimeFormatter.format_time_vtt(start_time)} --> "
            f"{SubtitleTimeFormatter.format_time_vtt(end_time)}"
        )

        # Add positioning if enabled
        if self.config.enable_positioning:
            position_settings = []

            if self.config.line_position:
                position_settings.append(f"line:{self.config.line_position}")

            if self.config.text_align:
                position_settings.append(f"align:{self.config.text_align}")

            if position_settings:
                timecode += f" {','.join(position_settings)}"

        # Apply word wrapping
        if self.config.wrap_limit:
            text = self._wrap_text(text, self.config.wrap_limit)

        return f"{index}\n{timecode}\n{text}"

    @staticmethod
    def _wrap_text(text: str, max_chars: int) -> str:
        """Basic word wrapping implementation for subtitle text"""
        words = text.split()
        lines = []
        current_line = []
        current_length = 0

        for word in words:
            if current_length + len(word) + 1 > max_chars:  # +1 for space
                lines.append(" ".join(current_line))
                current_line = [word]
                current_length = len(word)
            else:
                current_line.append(word)
                current_length += len(word) + 1

        if current_line:
            lines.append(" ".join(current_line))

        return "\n".join(lines)

    def format_transcript(
            self,
            transcript: TranslatedTranscript | TranscriptionVerbose
    ) -> FormattedSubtitleStringsByVariant:
        """Format transcript into WebVTT with variants"""
        self.validate_segments(transcript.segments)
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
    def _has_romanization(transcript: TranslatedTranscript) -> bool:
        """Check if romanization is available"""
        return (transcript.translated_language.romanization_method and
                transcript.translated_language.romanization_method.lower() != "none")

    def _format_transcription(self, transcript: TranscriptionVerbose) -> str:
        """Format basic transcription with VTT header"""
        vtt_content = [self._build_vtt_header()]
        for index, segment in enumerate(transcript.segments, 1):
            vtt_content.append(
                self._format_segment(
                    index=index,
                    start_time=segment.start,
                    end_time=segment.end,
                    text=segment.text.strip()
                )
            )
        return "\n\n".join(vtt_content)

    def _format_translation(self, transcript: TranslatedTranscript) -> str:
        """Format translated text only"""
        vtt_content = [self._build_vtt_header()]
        for index, segment in enumerate(transcript.segments, 1):
            text = segment.translated_text.translated_text.strip()
            vtt_content.append(
                self._format_segment(
                    index=index,
                    start_time=segment.start,
                    end_time=segment.end,
                    text=text
                )
            )
        return "\n\n".join(vtt_content)

    def _format_romanized(self, transcript: TranslatedTranscript) -> str:
        """Format with translated text and italicized romanization"""
        vtt_content = [self._build_vtt_header()]
        for index, segment in enumerate(transcript.segments, 1):
            translated = segment.translated_text.translated_text.strip()
            romanized = segment.translated_text.romanized_text.strip()
            text = f"{translated}\n<i>{romanized}</i>"
            vtt_content.append(
                self._format_segment(
                    index=index,
                    start_time=segment.start,
                    end_time=segment.end,
                    text=text
                )
            )
        return "\n\n".join(vtt_content)

    def _format_multi_language(self, transcript: TranslatedTranscript) -> str:
        """Format with original, translated, and romanized (if available)"""
        vtt_content = [self._build_vtt_header()]
        for index, segment in enumerate(transcript.segments, 1):
            parts = [
                f"【Original】{segment.original_segment_text.strip()}",
                f"【Translated】{segment.translated_text.translated_text.strip()}"
            ]

            if self._has_romanization(transcript):
                parts.append(
                    f"【Romanized】<i>{segment.translated_text.romanized_text.strip()}</i>"
                )

            text = "\n".join(parts)
            vtt_content.append(
                self._format_segment(
                    index=index,
                    start_time=segment.start,
                    end_time=segment.end,
                    text=text
                )
            )
        return "\n\n".join(vtt_content)
