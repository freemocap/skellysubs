import logging

from skellysubs.core.audio_transcription.whisper_transcript_result_model import WhisperTranscriptionResult
from skellysubs.core.translation_pipeline.models.translated_transcript_model import \
    OldTranslatedTranscription
from skellysubs.core.translation_pipeline.translation_subtasks.match_translated_words import \
    word_level_translation_and_matching
from skellysubs.core.translation_pipeline.translation_subtasks.translate_full_text import text_translation
from skellysubs.core.translation_pipeline.translation_subtasks.translate_transcript_segments import \
    transcript_translation

logger = logging.getLogger(__name__)







async def translate_transcription_pipeline(og_transcription: WhisperTranscriptionResult,
                                           original_language: str = "ENGLISH"
                                           ) -> OldTranslatedTranscription:
    initialized_transcription = OldTranslatedTranscription.initialize(og_transcription=og_transcription,
                                                                      original_langauge=original_language)
    full_text_translated_transcript = await text_translation(initialized_transcription=initialized_transcription)

    segment_level_translated_transcript = await transcript_translation(
        full_text_translated_transcript=full_text_translated_transcript)

    translated_transcript_with_words = await word_level_translation_and_matching(
        segment_level_translated_transcript=segment_level_translated_transcript)

    return translated_transcript_with_words


