import logging
from datetime import timedelta
from pathlib import Path

from pydantic import BaseModel, Field

from skellysubs.core.audio_transcription.whisper_transcript_result_model import \
    WhisperTranscriptionResult
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

        for language_name in self.translations.translations.keys():
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

        for language_name in self.translations.translations.keys():
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
                                                        language="ENGLISH")

    translated_transcription = TranslatedTranscription.initialize(og_transcription=outer_og_transcription,
                                                                  original_langauge="ENGLISH")

    print(f"INITIALIZED TRANSLATED TRANSCRIPTION")
    pprint(translated_transcription.model_dump(), indent=2)

    print(f"TRANSLATED TRANSCRIPT JSON MODEL SCHEMA")
    pprint(translated_transcription.model_json_schema(), indent=2)
