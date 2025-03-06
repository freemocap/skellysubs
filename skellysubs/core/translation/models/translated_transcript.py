from pydantic import BaseModel

from skellysubs.core.translation.language_configs.language_configs import LanguageConfig
from skellysubs.core.translation.models.translated_text import TranslatedText
from skellysubs.core.translation.models.translated_transcript_segment import TranslatedTranscriptSegment
from skellysubs.core.translation.models.translation_typehints import LanguageNameString


class TranslatedTranscript(BaseModel):
    original_full_text: str
    translated_full_text: TranslatedText
    original_language: LanguageNameString
    translated_language: LanguageConfig
    translated_segments: list[TranslatedTranscriptSegment]
