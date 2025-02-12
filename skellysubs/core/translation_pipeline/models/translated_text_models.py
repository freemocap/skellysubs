import jieba
from pydantic import BaseModel, Field

from skellysubs.core.translation_pipeline.models.load_language_models import LanguageConfig, \
    get_language_configs
from skellysubs.core.translation_pipeline.models.translation_typehints import TranslatedTextString, \
    LanguageNameString, RomanizationMethodString, RomanizedTextString, NOT_TRANSLATED_YET_TEXT
from skellysubs.utilities.strip_punctuation_and_whitespace import strip_punctuation_and_whitespace


class TranslatedText(BaseModel):
    translated_text: TranslatedTextString = Field(
        description="The translated text in the target language, using the target language's script, characters, and/or alphabet")
    romanized_text: RomanizedTextString = Field(
        description="The romanized version of the translated text, if applicable")

    translated_language_name: LanguageNameString = Field(description="The name of the target language")
    romanization_method: RomanizationMethodString = Field(
        description="The method used to romanize the translated text, if applicable")

    @classmethod
    def initialize(cls, language_config: LanguageConfig):
        return cls(translated_text=NOT_TRANSLATED_YET_TEXT,
                   translated_language_name=language_config.language_name,
                   romanization_method=language_config.romanization_method,
                   romanized_text=NOT_TRANSLATED_YET_TEXT)

    def get_word_list(self) -> tuple[list[str], list[str] | None]:
        if "chinese" in self.translated_language_name.lower():
            return self._split_chinese(), self.romanized_text.split()
        return self.translated_text.split(), self.romanized_text.split()

    def _split_chinese(self) -> list[str]:
        if "chinese" not in self.translated_language_name.lower():
            raise ValueError(f"This method is for Chinese language text, received {self.translated_language_name}")
        # the jieba docs refer to 'cut_all=False' as 'Accrurate Mode', so I guess we should use it?
        split_characters_og = [character for character in jieba.cut(self.translated_text, cut_all=False)]

        stripped_characters = [strip_punctuation_and_whitespace(character) for character in split_characters_og]
        cleaned_characters = [character for character in stripped_characters if character != ""]

        return cleaned_characters


class TranslationsCollection(BaseModel):
    translations: dict[LanguageNameString, TranslatedText] = Field(default_factory=dict)

    @property
    def has_translations(self) -> bool:
        return not any(
            [translation.translated_text == NOT_TRANSLATED_YET_TEXT for translation in self.translations.values()])

    @classmethod
    def create(cls, original_language: LanguageNameString):
        # Load language configurations from YAML
        translations = {language.lower(): TranslatedText.initialize(language_config=config)
                        for language, config in get_language_configs().items() if
                        original_language.lower() not in language.lower()}
        return cls(translations=translations)

    def languages_and_romanizations(self) -> dict[LanguageNameString, RomanizationMethodString]:
        return {language: text.romanization_method for language, text in self.translations.items()}
