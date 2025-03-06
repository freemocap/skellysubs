from enum import Enum
from typing import Type

from skellysubs.core.subtitles.base_subtitle_formatter import SubtitleFormatter
from skellysubs.core.subtitles.srt_subtitle_formatter import SRTFormatter


class SubtitleFormat(Enum):
    SRT = "srt"
    # Add more formats as needed
    # VTT = "vtt"
    # SSA = "ssa"


class SubtitleFormatterFactory:
    _formatters = {
        SubtitleFormat.SRT: SRTFormatter
    }

    @classmethod
    def get_formatter(cls, format_type: SubtitleFormat) -> SubtitleFormatter:
        formatter_class = cls._formatters.get(format_type)
        if not formatter_class:
            raise ValueError(f"Unsupported subtitle format: {format_type}")
        return formatter_class()