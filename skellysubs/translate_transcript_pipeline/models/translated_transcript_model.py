import jieba
from pydantic import BaseModel, Field

from skellysubs.audio_transcription.whisper_transcript_result_full_model import \
    WhisperTranscriptionResult, WhisperWordTimestamp
from skellysubs.translate_transcript_pipeline.models.language_models import LanguageNames, LanguagePairs, \
    LanguagePair
from skellysubs.translate_transcript_pipeline.models.translation_typehints import NOT_TRANSLATED_YET_TEXT, \
    LanguageNameString, RomanizationMethodString, RomanizedTextString, TranslatedTextString, OriginalTextString, \
    StartingTimestamp, EndingTimestamp
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

    @property
    def has_translations(self) -> bool:
        return not any([translation['translated_text'] == NOT_TRANSLATED_YET_TEXT
                        for translation in self.model_dump().values()])

    @classmethod
    def create(cls):
        return cls(english=TranslatedText.initialize(LanguagePair.from_enum(LanguagePairs.ENGLISH)),
                   spanish=TranslatedText.initialize(LanguagePair.from_enum(LanguagePairs.SPANISH)),
                   chinese=TranslatedText.initialize(LanguagePair.from_enum(LanguagePairs.CHINESE_MANDARIN_SIMPLIFIED)),
                   arabic=TranslatedText.initialize(LanguagePair.from_enum(LanguagePairs.ARABIC_LEVANTINE)))

    def languages_and_romanizations(self) -> dict[LanguageNameString, RomanizationMethodString]:
        return {LanguageNames.ENGLISH.value: self.english.romanization_method,
                LanguageNames.SPANISH.value: self.spanish.romanization_method,
                LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value: self.chinese.romanization_method,
                LanguageNames.ARABIC_LEVANTINE.value: self.arabic.romanization_method}

    def get_word_list_by_language(self, language: LanguageNameString) -> tuple[list[str], list[str] | None]:
        if language.lower() in LanguageNames.ENGLISH.value.lower():
            return self.english.get_word_list()
        if language.lower() in LanguageNames.SPANISH.value.lower():
            return self.spanish.get_word_list()
        if language.lower() in LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
            return self.chinese.get_word_list()
        if language.lower() in LanguageNames.ARABIC_LEVANTINE.value.lower():
            return self.arabic.get_word_list()
        else:
            raise ValueError(f"Language {language} not found in the translations collection.")


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

    @property
    def language_name(self) -> str:
        return self.target_language_pair.language


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

    @property
    def target_languages(self) -> dict[LanguageNameString, RomanizationMethodString]:
        return self.translations.languages_and_romanizations()


class TranslatedWhisperWordTimestamp(BaseModel):
    start: StartingTimestamp = Field(
        description="The start time of the period in the segment when the word was spoken, in seconds since the start of the recording. Should match the end time of the previous word in the segment or the start time of the segment for the first word.")
    end: EndingTimestamp = Field(
        description="The end time of the period in the recording when the word was spoken, in seconds since the start of the recording. Should match the start time of the next word in the segment or the end time of the segment for the last word.")
    original_word: OriginalTextString = Field(
        description="The original word spoken in the segment, in its original language")
    # translations: TranslationsCollection = Field(
    #     description="The translations of the original word into the target languages with their romanizations")
    matched_words: dict[LanguageNameString, MatchedTranslatedWord] = Field(
        description="The translated words in each target language, with their romanizations")

    # word_type: WordTypeSchemas|str = Field(default=WordTypeSchemas.OTHER.name,
    #                                    description="Linguistic features of the word, such as part of speech, tense, etc.")

    @classmethod
    def from_whisper_result(cls, word: WhisperWordTimestamp):
        return cls(start=word.start,
                   end=word.end,
                   original_word=word.word,
                   matched_words={},
                   )
        # word_type=WordTypeSchemas.NOT_PROCESSED.name)

    def get_word_by_language(self, language: LanguageNameString) -> tuple[str, str | None]:
        if language.lower() in LanguageNames.ENGLISH.value.lower() or language.lower() in "original_text" or language.lower() in "original_word" or language.lower() in "original":
            return self.original_word, None
        if language.lower() in LanguageNames.SPANISH.value.lower():
            return self.translations.spanish.translated_text, None
        if language.lower() in LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
            return self.translations.chinese.translated_text, self.translations.chinese.romanized_text
        if language.lower() in LanguageNames.ARABIC_LEVANTINE.value.lower():
            return self.translations.arabic.translated_text, self.translations.arabic.romanized_text
        else:
            raise ValueError(f"Language {language} not found in the translations collection.")


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

    @property
    def target_languages(self) -> dict[LanguageNameString, RomanizationMethodString]:
        return self.translations.languages_and_romanizations()

    @property
    def og_text_and_translations(self) -> dict[str, dict[str, str]]:
        return {LanguageNames.ENGLISH.value: {'text': self.original_segment_text, 'romanization': None},
                LanguageNames.SPANISH.value: {'text': self.translations.spanish.translated_text,
                                              'romanization': None},
                LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value: {'text': self.translations.chinese.translated_text,
                                                                  'romanziation': self.translations.chinese.romanized_text},
                LanguageNames.ARABIC_LEVANTINE.value: {'text': self.translations.arabic.translated_text,
                                                       'romanziation': self.translations.arabic.romanized_text}}

    def get_text_by_language(self, language: LanguageNameString) -> tuple[str, str | None]:
        language_lower = language.lower()
        match language_lower:
            case _ if language_lower in LanguageNames.ENGLISH.value.lower() or language_lower in {"original_text",
                                                                                                  "original_word",
                                                                                                  "original"}:
                return self.original_segment_text, None
            case _ if language_lower in LanguageNames.SPANISH.value.lower():
                return self.translations.spanish.translated_text, None
            case _ if language_lower in LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
                return self.translations.chinese.translated_text, self.translations.chinese.romanized_text
            case _ if language_lower in LanguageNames.ARABIC_LEVANTINE.value.lower():
                return self.translations.arabic.translated_text, self.translations.arabic.romanized_text
            case _:
                raise ValueError(f"Language {language} not found in the translations collection.")

    def set_translation_by_language(self, language: LanguageNameString,
                                    translation: TranslatedText):
        language_lower = language.lower()
        match language_lower:
            case _ if language_lower in LanguageNames.ENGLISH.value.lower() or language_lower in {"original_text",
                                                                                                  "original_word",
                                                                                                  "original"}:
                self.translations.english = translation
            case _ if language_lower in LanguageNames.SPANISH.value.lower():
                self.translations.spanish = translation
            case _ if language_lower in LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
                self.translations.chinese = translation
            case _ if language_lower in LanguageNames.ARABIC_LEVANTINE.value.lower():
                self.translations.arabic = translation
            case _:
                raise ValueError(f"Language {language} not found in the translations collection.")

    @property
    def original_words(self) -> str:
        return ', \n'.join([word.model_dump_json(indent=2) for word in self.words])

    def get_word_list_by_language(self, language: LanguageNameString) -> tuple[list[str], list[str] | None]:
        language_lower = language.lower()
        match language_lower:
            case _ if language_lower in LanguageNames.ENGLISH.value.lower() or language_lower in {"original_text",
                                                                                                  "original_word",
                                                                                                  "original"}:
                return [word.original_word for word in self.words], None
            case _ if language_lower in LanguageNames.SPANISH.value.lower():
                return self.translations.get_word_list_by_language(language=language_lower)
            case _ if language_lower in LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
                return self.translations.get_word_list_by_language(language=language_lower)
            case _ if language_lower in LanguageNames.ARABIC_LEVANTINE.value.lower():
                return self.translations.get_word_list_by_language(language=language_lower)
            case _:
                raise ValueError(f"Language {language} not found in the translations collection.")

    def get_indexed_word_list_by_language(self, language: LanguageNameString, strip_punctuation: bool = True) -> list[
        str]:
        word_list, _ = self.get_word_list_by_language(language)
        if strip_punctuation:
            word_list = [strip_punctuation_and_whitespace(word) for word in word_list]

        return [f"([translated-language-index-{index}]{word.strip()})" for index, word in enumerate(word_list)]

    @property
    def original_segment_text_with_words_indexed(self) -> str:
        return ' '.join([f" ([{index}]{word.original_word}) " for index, word in enumerate(self.words)])

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


class TranslatedTranscription(BaseModel):
    original_text: OriginalTextString = Field(
        description="The original text of the transcription in its original language")
    original_language: LanguageNameString = Field(description="The name of the original language of the transcription")
    translations: TranslationsCollection = Field(
        description="The translations of the original text into the target languages with their romanizations")
    segments: list[TranscriptSegment] = Field(
        description="Timestamped segments of the original text, with translations and romanizations (excluding word-level timestamps)")

    @property
    def translated_languagues(self) -> list[LanguageNameString]:
        return [language for language in self.translations.languages_and_romanizations().keys()]

    @property
    def target_languages_as_string(self) -> str:
        return ', '.join([f"Language: {language} (Romanization method: {romanization})" for language, romanization in
                          self.translations.languages_and_romanizations().items()])

    @property
    def has_translations(self) -> bool:
        return self.translations.has_translations

    @property
    def og_text_and_translations(self) -> dict[str, dict[str, str]]:
        return {LanguageNames.ENGLISH.value: {'text': self.original_text, 'romanization': None},
                LanguageNames.SPANISH.value: {'text': self.translations.spanish.translated_text, 'romanization': None},
                LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value: {'text': self.translations.chinese.translated_text,
                                                                  'romanziation': self.translations.chinese.romanized_text},
                LanguageNames.ARABIC_LEVANTINE.value: {'text': self.translations.arabic.translated_text,
                                                       'romanziation': self.translations.arabic.romanized_text}}

    @classmethod
    def initialize(cls,
                   og_transcription: WhisperTranscriptionResult,
                   original_langauge: LanguageNameString = LanguageNames.ENGLISH.value,
                   ):
        segments = []
        for segment in og_transcription.segments:
            segments.append(TranscriptSegment(original_segment_text=segment.text,
                                              original_language=original_langauge,
                                              translations=TranslationsCollection.create(),
                                              start=segment.start,
                                              end=segment.end,
                                              matched_translated_segment_by_language={},
                                              words=[TranslatedWhisperWordTimestamp.from_whisper_result(word) for word
                                                     in
                                                     segment.words]
                                              )
                            )
        return cls(original_text=og_transcription.text,
                   original_language=LanguageNames.ENGLISH.value,
                   translations=TranslationsCollection.create(),
                   segments=segments)

    def language_pair_by_language(self, language: LanguageNameString) -> LanguagePair:
        language_lower = language.lower()
        match language_lower:
            case _ if language_lower in LanguageNames.ENGLISH.value.lower() or language_lower in {"original_text",
                                                                                                  "original_word",
                                                                                                  "original"}:
                return LanguagePair.from_enum(LanguagePairs.ENGLISH)
            case _ if language_lower in LanguageNames.SPANISH.value.lower():
                return LanguagePair.from_enum(LanguagePairs.SPANISH)
            case _ if language_lower in LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
                return LanguagePair.from_enum(LanguagePairs.CHINESE_MANDARIN_SIMPLIFIED)
            case _ if language_lower in LanguageNames.ARABIC_LEVANTINE.value.lower():
                return LanguagePair.from_enum(LanguagePairs.ARABIC_LEVANTINE)
            case _:
                raise ValueError(f"Language {language} not found in the translations collection.")

    @property
    def target_laguage_pairs(self) -> dict[LanguageNameString, LanguagePair]:
        return {language: self.language_pair_by_language(language) for language in self.translated_languagues}

    # @classmethod
    # def from_segment_level_translation(cls,
    #                                    og_transcription: WhisperTranscriptionResult,
    #                                    segment_level_translated_transcript: TranslatedText):
    #     segments = []
    #     for og_segment, translated_segment in zip(og_transcription.segments,
    #                                               segment_level_translated_transcript.segments):
    #         segments.append(SegmentWithWords(original_segment_text=og_segment.text,
    #                                          translations=translated_segment.translations,
    #                                          start=og_segment.start,
    #                                          end=og_segment.end,
    #                                          words=[
    #                                                                  TranslatedWhisperWordTimestamp.from_whisper_result(
    #                                                                      word) for word in
    #                                                                  og_segment.words]
    #                                          )
    #                         )
    #     return cls(original_text=og_transcription.text,
    #                original_language=og_transcription.language,
    #                translations=segment_level_translated_transcript.translations,
    #                segments=segments)

    def get_matched_segment_and_word_at_timestamp(self, timestamp: float) -> CurrentSegmentAndMatchedWord:
        """
        Get the segment and word that contains the given timestamp (in seconds since the start of the recording)
        """
        current_segment: TranscriptSegment | None = None
        current_word: TranslatedWhisperWordTimestamp | None = None
        matched_translated_segments: dict[LanguageNames, MatchedTranslatedSegment] = {}
        matched_translated_words: dict[LanguageNames, MatchedTranslatedWord] = {}

        for segment_number, segment in enumerate(self.segments):
            relevant_segment_start_time = segment.start
            relevant_segment_end_time = self.segments[segment_number + 1].start if segment_number < len(
                self.segments) - 1 else segment.end

            if relevant_segment_start_time <= timestamp <= relevant_segment_end_time:
                current_segment = segment
                break
        if current_segment is None:
            current_segment = self.segments[-1]

        for word_number, word in enumerate(current_segment.words):
            relevant_word_start_time = word.start
            relevant_word_end_time = current_segment.words[word_number + 1].start if word_number < len(
                current_segment.words) - 1 else word.end
            if relevant_word_start_time <= timestamp <= relevant_word_end_time:
                current_word = word

        if current_word is None:
            current_word = current_segment.words[-1]

        # Find the matching segment and word in each of the target languages

        for language, matched_segment in current_segment.matched_translated_segment_by_language.items():
            matched_translated_segments[language] = matched_segment
            for matched_translated_word_number, matched_translated_word in enumerate(
                    matched_segment.matched_translated_words):
                relevant_word_start_time = matched_translated_word.start_time
                relevant_word_end_time = matched_segment.matched_translated_words[
                    matched_translated_word_number + 1].start_time if matched_translated_word_number < len(
                    matched_segment.matched_translated_words) - 1 else matched_translated_word.end_time
                if relevant_word_start_time <= timestamp <= relevant_word_end_time:
                    matched_translated_words[language] = matched_translated_word

            if not matched_translated_words.get(language):
                matched_translated_words[language] = matched_segment.matched_translated_words[-1]
        return CurrentSegmentAndMatchedWord(current_segment=current_segment,
                                            current_word=current_word,
                                            matched_segment_by_language=matched_translated_segments,
                                            matched_word_by_language=matched_translated_words)


if __name__ == '__main__':
    from pprint import pprint

    outer_og_transcription = WhisperTranscriptionResult(text="This is a test transcription, wowee zoowee",
                                                        segments=[],
                                                        language=LanguageNames.ENGLISH.value)

    translated_transcription = TranslatedTranscription.initialize(og_transcription=outer_og_transcription)

    print(f"INITIALIZED TRANSLATED TRANSCRIPTION")
    pprint(translated_transcription.model_dump(), indent=2)

    print(f"TRANSLATED TRANSCRIPT JSON MODEL SCHEMA")
    pprint(translated_transcription.model_json_schema(), indent=2)
