import jieba
from pydantic import BaseModel, Field

from skellysubs.core.translation.language_configs.language_configs import LanguageConfig
from skellysubs.core.translation.models.translation_typehints import TranslatedTextString, RomanizedTextString, \
    LanguageNameString, RomanizationMethodString, NOT_TRANSLATED_YET_TEXT, OriginalLanguageTextString, \
    LanguageISO6391CodeString
from skellysubs.utilities.strip_punctuation_and_whitespace import strip_punctuation_and_whitespace

class BaseTextModel(BaseModel):
    text: str = Field(description="The text in the source language")
    language_name: LanguageNameString = Field(description="The name of the language")


class OriginalLanguageTextModel(BaseTextModel):
    text: OriginalLanguageTextString = Field(description="The text in the source language")

class TranslatedTextModel(BaseTextModel):
    text: TranslatedTextString = Field(
        description="The translated text in the target language, using the target language's script, characters, and/or alphabet")
    romanized_text: RomanizedTextString = Field(
        description="The romanized version of the translated text, if applicable")
    language_name : LanguageNameString = Field(description="The name of the target language")
    romanization_method: RomanizationMethodString = Field(
        description="The method used to romanize the translated text, if applicable")
    original_text: OriginalLanguageTextModel = Field(description="The original text in the source language")

    def get_word_list(self) -> tuple[list[str], list[str] | None]:
        if "chinese" in self.translated_language_name.lower():
            return self._split_chinese(), self.romanized_text.split()
        return self.translated_text.split(), self.romanized_text.split()

# def _split_chinese_words() -> list[str]:
#     if "chinese" not in self.translated_language_name.lower():
#         raise ValueError(f"This method is for Chinese language text, received {self.translated_language_name}")
#     # the jieba docs refer to 'cut_all=False' as 'Accrurate Mode', so I guess we should use it?
#     split_characters_og = [character for character in jieba.cut(self.translated_text, cut_all=False)]
#
#     stripped_characters = [strip_punctuation_and_whitespace(character) for character in split_characters_og]
#     cleaned_characters = [character for character in stripped_characters if character != ""]
#
#     return cleaned_characters
