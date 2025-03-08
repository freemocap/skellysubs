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

    @property
    def segments(self):
        """
        Helper so we can use the same api as the TranslationVerbose class
        """
        return self.translated_segments

    @property
    def text(self):
        return self.translated_full_text.translated_text

    @property
    def language(self):
        return self.translated_language.language_name