from pydantic import BaseModel, Field

from skellysubs.skellysubs_core.audio_transcription.whisper_transcript_result_model import WhisperWordTimestamp
from skellysubs.skellysubs_core.translation_pipeline.models.language_models import LanguageNames, LanguagePair
from skellysubs.skellysubs_core.translation_pipeline.models.translated_text_models import TranslationsCollection, \
    TranslatedText
from skellysubs.skellysubs_core.translation_pipeline.models.translation_typehints import StartingTimestamp, \
    EndingTimestamp, OriginalTextString, TranslatedTextString, RomanizedTextString, LanguageNameString
from skellysubs.utilities.strip_punctuation_and_whitespace import strip_punctuation_and_whitespace


class MatchedTranslatedWord(BaseModel):
    original_language: LanguageNames = Field(description="The original language of the word from the original segment")
    start_time: StartingTimestamp = Field(
        description="The start time of the period in the segment when the word was spoken, in seconds since the start of the recording. Should match the end time of the previous word in the segment or the start time of the segment for the first word.")
    end_time: EndingTimestamp = Field(
        description="The end time of the period in the recording when the word was spoken, in seconds since the start of the recording. Should match the start time of the next word in the segment or the end time of the segment for the last word.")
    target_language_pair: LanguagePair = Field(
        description="The target language pair for the translation, including any romanization methods if applicable")
    original_word_text: str = Field(description="The original word from the original segment")
    original_word_index: int = Field(description="The index of the original word in the original segment")

    translated_word_text: str = Field(
        description="The translated word from the target language translation that match the original word")
    translated_word_romanized_text: str | None = Field(default=None,
                                                       description="If this is a non-latin alphabet languages, this holds the translated word from the target language translation that match the original word")
    translated_word_index: int = Field(
        description="The index of the translated word in the translated segment that match the word in the original segment")


class MatchedTranslatedSegment(BaseModel):
    start: StartingTimestamp = Field(
        description="The start time of the period in the recording when the segment was spoken in seconds since the start of the recording. Should match the end time of the previous segment or the start time of the recording for the first segment.")
    end: EndingTimestamp = Field(
        description="The end time of the segment in the recording when the segment was spoken in seconds since the start of the recording. Should match the start time of the next segment or the end time of the recording for the last segment.")

    target_language_pair: LanguagePair = Field(
        description="The target language pair for the translation, including any romanization methods if applicable")

    original_segment_text: OriginalTextString = Field(
        description="The original text of the segment in its original language")
    translated_segment_text: TranslatedTextString = Field(
        description="The translated text of the segment in the target language, using the target language's script, characters, and/or alphabet")
    romanized_translated_text: RomanizedTextString | None = Field(default=None,
                                                                  description="The romanized version of the translated text, if applicable")

    original_words_list: list[str] = Field(
        description="The original words in the segment, with the index in the list matching the index of the word in the segment string")
    translated_words_list: list[str] = Field(
        description="The words or characters in the translated segment that match the original words in the segment - may or may not be the same length as the original_words_list (because of differences between the  languages)")
    romanized_translated_words_list: list[str] | None = Field(default=None,
                                                              description="The romanized versions of the translated words in the segment, if applicable - must be the same length as the translated_words_list")
    matched_translated_words: list[MatchedTranslatedWord] = Field(
        description="The translated words in the segment, with their romanizations if applicable - must be the same length as the original_words_list")


class TranslatedWhisperWordTimestamp(BaseModel):
    start: StartingTimestamp = Field(
        description="The start time of the period in the segment when the word was spoken, in seconds since the start of the recording. Should match the end time of the previous word in the segment or the start time of the segment for the first word.")
    end: EndingTimestamp = Field(
        description="The end time of the period in the recording when the word was spoken, in seconds since the start of the recording. Should match the start time of the next word in the segment or the end time of the segment for the last word.")
    original_word: OriginalTextString = Field(
        description="The original word spoken in the segment, in its original language")
    matched_words: dict[LanguageNameString, MatchedTranslatedWord] = Field(
        description="The translated words in each target language, with their romanizations")

    # TODO - would be cool to also extract linguistic features of the word, such as part of speech, tense, etc.
    # word_type: WordTypeSchemas|str = Field(default=WordTypeSchemas.OTHER.name,
    #                                    description="Linguistic features of the word, such as part of speech, tense, etc.")

    @classmethod
    def from_whisper_result(cls, word: WhisperWordTimestamp):
        return cls(start=word.start,
                   end=word.end,
                   original_word=word.word,
                   matched_words={},
                   )


class TranscriptSegment(BaseModel):
    original_segment_text: OriginalTextString = Field(
        description="The original text of the segment in its original language")
    original_language: LanguageNameString = Field(description="The name of the original language of the segment")
    translations: TranslationsCollection = Field(
        description="The translations of the original text into the target languages with their romanizations")
    start: StartingTimestamp = Field(
        description="The start time of the period in the recording when the segment was spoken in seconds since the start of the recording. Should match the end time of the previous segment or the start time of the recording for the first segment.")
    end: EndingTimestamp = Field(
        description="The end time of the segment in the recording when the segment was spoken in seconds since the start of the recording. Should match the start time of the next segment or the end time of the recording for the last segment.")
    matched_translated_segment_by_language: dict[str, MatchedTranslatedSegment] | None = Field(default=None,
                                                                                               description="The matched translated segment with the original segment")
    words: list[TranslatedWhisperWordTimestamp] = Field(
        description="The words in the segment, with their start and end times in the recording")

    def set_translation_by_language(self, language: LanguageNameString,
                                    translation: TranslatedText):
        language_lower = language.lower()
        match language_lower:
            case _ if language_lower in LanguageNames.ENGLISH.value.lower():
                self.translations.english = translation
            case _ if language_lower in LanguageNames.SPANISH.value.lower():
                self.translations.spanish = translation
            case _ if language_lower in LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
                self.translations.chinese = translation
            case _ if language_lower in LanguageNames.ARABIC_LEVANTINE.value.lower():
                self.translations.arabic = translation
            case _ if language_lower in LanguageNames.HINDI.value.lower():
                self.translations.hindi = translation
            case _:
                raise ValueError(f"Language {language} not found in the translations collection.")

    def get_word_list_by_language(self, language: LanguageNameString) -> tuple[list[str], list[str] | None]:
        if language.lower() in LanguageNames.ENGLISH.value.lower():
            return [word.original_word for word in self.words], None
        return self.translations.get_word_list_by_language(language=language.lower())

    def get_indexed_word_list_by_language(self, language: LanguageNameString, strip_punctuation: bool = True) -> list[
        str]:
        word_list, _ = self.get_word_list_by_language(language)
        if strip_punctuation:
            word_list = [strip_punctuation_and_whitespace(word) for word in word_list]

        return [f"([translated-language-index-{index}]{word.strip()})" for index, word in enumerate(word_list)]

    @property
    def original_segment_text_with_words_indexed_and_timestamped(self) -> str:
        return ' '.join([
            f" ([orginal-language-index-{index}]{word} [starting_timestamp: {word.start}, ending_timestamp: {word.end}])\n"
            for index, word in enumerate(self.words)])


class CurrentSegmentAndMatchedWord(BaseModel):
    current_segment: TranscriptSegment | None = None
    current_word: TranslatedWhisperWordTimestamp | None = None
    matched_segment_by_language: dict[LanguageNames, MatchedTranslatedSegment] | None = None
    matched_word_by_language: dict[str, MatchedTranslatedWord] | None = None
