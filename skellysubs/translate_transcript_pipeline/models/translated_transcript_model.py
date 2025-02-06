import logging
from datetime import timedelta
from pathlib import Path

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

logger = logging.getLogger(__name__)


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
            case _:
                raise ValueError(f"Language {language} not found in the translations collection.")

    def get_word_list_by_language(self, language: LanguageNameString) -> tuple[list[str], list[str] | None]:
        language_lower = language.lower()
        match language_lower:
            case _ if language_lower in LanguageNames.ENGLISH.value.lower():
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
    def translated_languages(self) -> list[LanguageNameString]:
        return [language for language in self.translations.languages_and_romanizations().keys()]

    @property
    def language_pairs(self) -> dict[LanguageNameString, LanguagePair]:
        return {language: self.language_pair_by_language(language) for language in self.translated_languages}

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
        return {language: self.language_pair_by_language(language) for language in self.translated_languages}

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

    def generate_subtitle_files(self, file_basename: str, subtitle_directory: str):
        logger.info(f"Generating subtitle files for {file_basename} in {subtitle_directory}")
        self.generate_srt_files(file_basename, str(Path(subtitle_directory) / 'srt'))
        self.generate_vtt_files(file_basename, str(Path(subtitle_directory) / 'vtt'))
        self.generate_ttml_files(file_basename, str(Path(subtitle_directory) / 'ttml'))

    def generate_srt_files(self, file_basename: str, save_directory: str):
        """Generate SRT file content from a TranslatedTranscription object."""
        logger.info(f"Generating SRT files for {file_basename} in {save_directory}")
        save_directory = Path(save_directory)
        save_directory.mkdir(parents=True, exist_ok=True)

        def format_srt_time(seconds: float) -> str:
            """Convert seconds to SRT timestamp format."""
            if seconds <= 0:
                seconds = 0.1
            td = timedelta(seconds=seconds)
            return str(td)[:-3].replace('.', ',')

        def create_srt_block(index, start, end, word_list, romanized_word_list=None):
            start_time = format_srt_time(start)
            end_time = format_srt_time(end)
            srt_block = f"{index}\n{start_time} --> {end_time}\n{' '.join(word_list)}\n"
            if romanized_word_list:
                srt_block += ' '.join(romanized_word_list) + '\n'
            return srt_block

        for language_name in self.og_text_and_translations.keys():
            srt_content = []
            str_content_with_romanization = []
            for index, segment in enumerate(self.segments, start=1):
                segment_word_list, romanized_word_list = segment.get_word_list_by_language(language_name)
                srt_content.append(create_srt_block(index, segment.start, segment.end, segment_word_list))
                if romanized_word_list:
                    str_content_with_romanization.append(create_srt_block(index, segment.start, segment.end,
                                                                         segment_word_list, romanized_word_list))
            srt_filename = save_directory / f"{file_basename}_{language_name}.srt"
            with open(srt_filename, 'w', encoding='utf-8') as f:
                f.write('\n\n'.join(srt_content))

            if str_content_with_romanization:
                srt_filename = save_directory / f"{file_basename}_{language_name}_with_romanization.srt"
                with open(srt_filename, 'w', encoding='utf-8') as f:
                    f.write('\n\n'.join(str_content_with_romanization))

    def generate_vtt_files(self, file_basename: str, save_directory: str):
        """Generate VTT file content from a TranslatedTranscription object."""
        logger.info(f"Generating VTT files for {file_basename} in {save_directory}")
        save_directory = Path(save_directory)
        save_directory.mkdir(parents=True, exist_ok=True)

        def format_vtt_time(seconds: float) -> str:
            """Convert seconds to VTT timestamp format."""
            if seconds <= 0:
                seconds = 0.1
            td = timedelta(seconds=seconds)
            return str(td)[:-3] + '.000'

        for language_name in self.og_text_and_translations.keys():
            vtt_content = ["WEBVTT\n"]
            vtt_content_with_romanization = ["WEBVTT\n"]
            for index, segment in enumerate(self.segments, start=1):
                start_time = format_vtt_time(segment.start)
                end_time = format_vtt_time(segment.end)
                segment_word_list, romanized_word_list = segment.get_word_list_by_language(language_name)

                vtt_block = f"{start_time} --> {end_time} line:50%\n{' '.join(segment_word_list)}\n"
                vtt_content.append(vtt_block)

                if romanized_word_list:
                    vtt_block_with_romanization = vtt_block + ' '.join(romanized_word_list) + '\n'
                    vtt_content_with_romanization.append(vtt_block_with_romanization)

            vtt_filename = save_directory / f"{file_basename}_{language_name}.vtt"
            with open(vtt_filename, 'w', encoding='utf-8') as f:
                f.write('\n'.join(vtt_content))

            if len(vtt_content_with_romanization) > 1:
                vtt_filename_with_romanization = save_directory / f"{file_basename}_{language_name}_with_romanization.vtt"
                with open(vtt_filename_with_romanization, 'w', encoding='utf-8') as f:
                    f.write('\n'.join(vtt_content_with_romanization))

    def generate_ttml_files(self, file_basename: str, save_directory: str):
        """Generate TTML file content from a TranslatedTranscription object."""
        logger.info(f"Generating TTML files for {file_basename} in {save_directory}")
        save_directory = Path(save_directory)
        save_directory.mkdir(parents=True, exist_ok=True)

        def format_ttml_time(seconds: float) -> str:
            """Convert seconds to TTML timestamp format."""
            if seconds <= 0:
                seconds = 0.1
            td = timedelta(seconds=seconds)
            hours, remainder = divmod(td.seconds, 3600)
            minutes, seconds = divmod(remainder, 60)
            return f"{hours:02}:{minutes:02}:{seconds:02}.{td.microseconds // 1000:03}"

        for language_name in self.og_text_and_translations.keys():
            ttml_content = [
                '<?xml version="1.0" encoding="UTF-8"?>',
                '<tt xmlns="http://www.w3.org/ns/ttml">',
                '<head>',
                '<layout>',
                '<region xml:id="bottomHalf" tts:origin="0% 50%" tts:extent="100% 50%"/>',
                '</layout>',
                '</head>',
                '<body><div>'
            ]
            ttml_content_with_romanization = ttml_content.copy()

            for index, segment in enumerate(self.segments, start=1):
                start_time = format_ttml_time(segment.start)
                end_time = format_ttml_time(segment.end)
                segment_word_list, romanized_word_list = segment.get_word_list_by_language(language_name)

                ttml_block = f'<p begin="{start_time}" end="{end_time}">{" ".join(segment_word_list)}</p>'
                ttml_content.append(ttml_block)

                if romanized_word_list:
                    ttml_block_with_romanization = (
                        f'<p begin="{start_time}" end="{end_time}">'
                        f'{" ".join(segment_word_list)}<br/>{" ".join(romanized_word_list)}</p>'
                    )
                    ttml_content_with_romanization.append(ttml_block_with_romanization)

            ttml_content.append('</div></body></tt>')
            ttml_filename = save_directory / f"{file_basename}_{language_name}.ttml"
            with open(ttml_filename, 'w', encoding='utf-8') as f:
                f.write('\n'.join(ttml_content))

            if len(ttml_content_with_romanization) > 3:  # Check if there are any entries beyond the headers
                ttml_content_with_romanization.append('</div></body></tt>')
                ttml_filename_with_romanization = save_directory / f"{file_basename}_{language_name}_with_romanization.ttml"
                with open(ttml_filename_with_romanization, 'w', encoding='utf-8') as f:
                    f.write('\n'.join(ttml_content_with_romanization))


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
