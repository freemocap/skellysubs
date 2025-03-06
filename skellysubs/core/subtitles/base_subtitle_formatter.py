from abc import ABC, abstractmethod
from typing import Dict

from openai.types.audio import TranscriptionVerbose

from skellysubs.core.translation.models.translated_transcript import TranslatedTranscript


class SubtitleFormatter(ABC):
    @abstractmethod
    def format_openai_transcript(self, transcript: TranscriptionVerbose) -> str:
        """Format original OpenAI/Whisper  verbose transcript into subtitle format"""
        pass

    @abstractmethod
    def format_translated_transcript(self, translated_transcript: TranslatedTranscript) -> Dict[str, str]:
        """Format translated transcript into subtitle format with variants"""
        pass