import logging
from pathlib import Path

from pydantic import BaseModel, Field

from skellysubs.core.audio_transcription.whisper_transcript_result_model import \
    WhisperTranscriptionResult
from skellysubs.core.subtitles.subtitle_generators import generate_vtt_files, generate_ttml_files
from skellysubs.core.translation_pipeline.models.translated_segment_models import MatchedTranslatedWord, \
    MatchedTranslatedSegment, TranslatedWhisperWordTimestamp, TranscriptSegment, CurrentSegmentAndMatchedWord
from skellysubs.core.translation_pipeline.models.translated_text_models import TranslationsCollection
from skellysubs.core.translation_pipeline.models.translation_typehints import LanguageNameString, \
    OriginalTextString

logger = logging.getLogger(__name__)


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
    def og_text_and_translations(self) -> dict[str, dict[str, str]]:
        ret = {self.original_language.lower(): {'text': self.original_text, 'romanization': None}}
        for language, translation in self.translations.translations.items():
            ret[language.lower()] = {'text': translation.translated_text, 'romanization': translation.romanized_text}
        return ret

    @classmethod
    def initialize(cls,
                   og_transcription: WhisperTranscriptionResult,
                   original_langauge: LanguageNameString):

        segments = []
        try:
            for segment in og_transcription.segments:
                segments.append(TranscriptSegment(original_segment_text=segment.text,
                                                  original_language=original_langauge,
                                                  translations=TranslationsCollection.create(original_language=original_langauge),
                                                  start=segment.start,
                                                  end=segment.end,
                                                  matched_translated_segment_by_language={},
                                                  words=[TranslatedWhisperWordTimestamp.from_whisper_result(word)
                                                         for word in segment.words] if segment.words else []
                                                  )
                                )
        except Exception as e:
            logger.error(f"Error initializing TranslatedTranscription: {e}")
            raise
        return cls(original_text=og_transcription.text,
                   original_language=original_langauge,
                   translations=TranslationsCollection.create(original_language=original_langauge),
                   segments=segments)


    def get_matched_segment_and_word_at_timestamp(self, timestamp: float) -> CurrentSegmentAndMatchedWord:
        """
        Get the segment and word that contains the given timestamp (in seconds since the start of the recording)
        """
        current_segment: TranscriptSegment | None = None
        current_word: TranslatedWhisperWordTimestamp | None = None
        matched_translated_segments: dict[LanguageNameString, MatchedTranslatedSegment] = {}
        matched_translated_words: dict[LanguageNameString, MatchedTranslatedWord] = {}

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
        generate_srt_formatted_subtitles(self, file_basename, str(Path(subtitle_directory) / 'srt'))
        generate_vtt_files(self, file_basename, str(Path(subtitle_directory) / 'vtt'))
        generate_ttml_files(self, file_basename, str(Path(subtitle_directory) / 'ttml'))


if __name__ == '__main__':
    from pprint import pprint

    outer_og_transcription = WhisperTranscriptionResult(text="This is a test transcription, wowee zoowee",
                                                        segments=[],
                                                        language="ENGLISH")

    translated_transcription = TranslatedTranscription.initialize(og_transcription=outer_og_transcription,
                                                                  original_langauge="ENGLISH")

    print(f"INITIALIZED TRANSLATED TRANSCRIPTION")
    pprint(translated_transcription.model_dump(), indent=2)

    print(f"TRANSLATED TRANSCRIPT JSON MODEL SCHEMA")
    pprint(translated_transcription.model_json_schema(), indent=2)
