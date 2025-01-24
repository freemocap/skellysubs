from enum import Enum

from pydantic import BaseModel, Field

from skellysubs.translate_transcript_pipeline.translation_typehints import LanguageNameString, \
    RomanizationMethodString


class LanguageNames(str, Enum):
    ENGLISH = "ENGLISH"
    SPANISH = "SPANISH"
    CHINESE_MANDARIN_SIMPLIFIED = "CHINESE_MANDARIN_SIMPLIFIED"
    ARABIC_LEVANTINE = "ARABIC_LEVANTINE"


class RomanizationMethods(str, Enum):
    """Enumeration of romanization methods with instructions."""
    NONE = "NONE - Do not apply any romanization method. Retain the original script."
    IPA = "IPA - Transcribe the text using the International Phonetic Alphabet (IPA) to provide precise pronunciation. Ensure accuracy in representing phonetic sounds"
    PINYIN = "PINYIN - Convert the text from Simplified Chinese to Pinyin, including tone markers. Maintain accuracy in reflecting the original Mandarin pronunciation."
    ALA_LC = "ALA-LC -  Use the American Library Association — Library of Congress (ALA-LC) romanization system for Arabic. Utilize full UTF-8 characters for diacritics and special characters to ensure precise representation. Adhere to standardized conventions for transliterating non-Latin scripts."
    DIN_31635 = "DIN-31635 - Apply the DIN 31635 standard for Arabic romanization. Ensure usage of full UTF-8 characters for all diacritics, following the precise guidelines for transliteration."


class LanguagePairs(tuple[LanguageNames, RomanizationMethods]):
    """Tuple pairing languages with their romanization methods."""
    ENGLISH = (LanguageNames.ENGLISH, RomanizationMethods.NONE)
    SPANISH = (LanguageNames.SPANISH, RomanizationMethods.NONE)
    CHINESE_MANDARIN_SIMPLIFIED = (LanguageNames.CHINESE_MANDARIN_SIMPLIFIED, RomanizationMethods.PINYIN)
    ARABIC_LEVANTINE = (LanguageNames.ARABIC_LEVANTINE, RomanizationMethods.ALA_LC)
    # Add additional pairs as needed


class LanguagePair(BaseModel):
    language: LanguageNameString = Field(description="The name of the target language")
    romanization_method: RomanizationMethodString = Field(
        description="The method used to romanize the translated text, if applicable")

    @classmethod
    def from_enum(cls, language_pair: tuple[LanguageNames, RomanizationMethods]):
        return cls(language=language_pair[0], romanization_method=language_pair[1])
