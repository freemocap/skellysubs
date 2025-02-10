import jieba
from pydantic import BaseModel, Field

from skellysubs.skellysubs_core.translation_pipeline.models.language_models import LanguagePair, LanguageNames, \
    LanguagePairs
from skellysubs.skellysubs_core.translation_pipeline.models.translation_typehints import TranslatedTextString, \
    LanguageNameString, RomanizationMethodString, RomanizedTextString, NOT_TRANSLATED_YET_TEXT
from skellysubs.utilities.strip_punctuation_and_whitespace import strip_punctuation_and_whitespace


class TranslatedText(BaseModel):
    translated_text: TranslatedTextString = Field(
        description="The translated text in the target language, using the target language's script, characters, and/or alphabet")
    translated_language: LanguageNameString = Field(description="The name of the target language")
    romanization_method: RomanizationMethodString = Field(
        description="The method used to romanize the translated text, if applicable")
    romanized_text: RomanizedTextString = Field(
        description="The romanized version of the translated text, if applicable")

    @classmethod
    def initialize(cls, language: LanguagePair):
        return cls(translated_text=NOT_TRANSLATED_YET_TEXT,
                   translated_language=language.language,
                   romanization_method=language.romanization_method,
                   romanized_text=NOT_TRANSLATED_YET_TEXT)

    def get_word_list(self) -> tuple[list[str], list[str] | None]:
        if self.translated_language.lower() in LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
            return self.split_chinese(), self.romanized_text.split()
        return self.translated_text.split(), self.romanized_text.split()

    def split_chinese(self) -> list[str]:
        if not self.translated_language.lower() in LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
            raise ValueError(f"Cannot split Chinese text for language {self.translated_language}")
        # the docs refer to 'cut_all=False' as 'Accrurate Mode', so I guess we should use it?
        split_characters_og = [character for character in jieba.cut(self.translated_text, cut_all=False)]

        stripped_characters = [strip_punctuation_and_whitespace(character) for character in split_characters_og]
        cleaned_characters = [character for character in stripped_characters if character != ""]

        return cleaned_characters


class TranslationsCollection(BaseModel):
    english: TranslatedText = Field(description="The original text in English")
    spanish: TranslatedText = Field(description="The translation of the original text into Spanish")
    chinese: TranslatedText = Field(description="The translation of the original text into Chinese Mandarin Simplified")
    arabic: TranslatedText = Field(description="The translation of the original text into Arabic Levantine")
    hindi: TranslatedText = Field(description="The translation of the original text into Hindi")

    @property
    def has_translations(self) -> bool:
        return not any([translation['translated_text'] == NOT_TRANSLATED_YET_TEXT
                        for translation in self.model_dump().values()])

    @classmethod
    def create(cls):
        return cls(english=TranslatedText.initialize(LanguagePair.from_enum(LanguagePairs.ENGLISH)),
                   spanish=TranslatedText.initialize(LanguagePair.from_enum(LanguagePairs.SPANISH)),
                   chinese=TranslatedText.initialize(LanguagePair.from_enum(LanguagePairs.CHINESE_MANDARIN_SIMPLIFIED)),
                   arabic=TranslatedText.initialize(LanguagePair.from_enum(LanguagePairs.ARABIC_LEVANTINE)),
                   hindi=TranslatedText.initialize(LanguagePair.from_enum(LanguagePairs.HINDI)))

    def languages_and_romanizations(self) -> dict[LanguageNameString, RomanizationMethodString]:
        return {LanguageNames.ENGLISH.value: self.english.romanization_method,
                LanguageNames.SPANISH.value: self.spanish.romanization_method,
                LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value: self.chinese.romanization_method,
                LanguageNames.ARABIC_LEVANTINE.value: self.arabic.romanization_method,
                LanguageNames.HINDI.value: self.hindi.romanization_method}

    def get_word_list_by_language(self, language: LanguageNameString) -> tuple[list[str], list[str] | None]:
        if language.lower() in LanguageNames.ENGLISH.value.lower():
            return self.english.get_word_list()
        if language.lower() in LanguageNames.SPANISH.value.lower():
            return self.spanish.get_word_list()
        if language.lower() in LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
            return self.chinese.get_word_list()
        if language.lower() in LanguageNames.ARABIC_LEVANTINE.value.lower():
            return self.arabic.get_word_list()
        if language.lower() in LanguageNames.HINDI.value.lower():
            return self.hindi.get_word_list()
        else:
            raise ValueError(f"Language {language} not found in the translations collection.")
