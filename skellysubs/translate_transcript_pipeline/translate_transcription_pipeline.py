import asyncio
import logging

from skellysubs.add_subtitles_to_video_pipeline.full_text_transcript_translation_prompt import \
    format_full_segement_level_transcript_translation_system_prompt
from skellysubs.add_subtitles_to_video_pipeline.segement_word_level_translation_prompt import \
    format_segment_word_level_transcript_translation_system_prompts
from skellysubs.ai_clients.openai_client.make_openai_json_mode_ai_request import \
    make_openai_json_mode_ai_request
from skellysubs.ai_clients.openai_client.openai_client import OPENAI_CLIENT, DEFAULT_LLM
from skellysubs.audio_transcription.whisper_transcript_result_full_model import WhisperTranscriptionResult
from skellysubs.translate_transcript_pipeline.models.translated_transcript_model import \
    TranslatedTranscriptionWithoutWords, \
    TranslatedTranscription, MatchedTranslatedSegment

logger = logging.getLogger(__name__)


async def translate_transcription_pipeline(og_transcription: WhisperTranscriptionResult,
                                           verbose: bool = True
                                           ) -> TranslatedTranscription:
    segment_level_translated_transcript = await full_text_and_segment_translation(og_transcription)

    translated_transcript_with_words = await segment_word_level_translation(og_transcription,
                                                                            segment_level_translated_transcript)

    return translated_transcript_with_words


async def segment_word_level_translation(og_transcription, segment_level_translated_transcript):
    # Word-level translation

    translated_transcript_with_words = TranslatedTranscription.from_segment_level_translation(
        og_transcription=og_transcription,
        segment_level_translated_transcript=segment_level_translated_transcript)

    prompts_by_segment_by_language = format_segment_word_level_transcript_translation_system_prompts(
        initialized_translated_transcript=translated_transcript_with_words)

    tasks = []
    result_addresses = []
    for language in translated_transcript_with_words.og_text_and_translations.keys():
        for segment_number, segment in enumerate(translated_transcript_with_words.segments):
            address = {'language': language, 'segment_number': segment_number}
            task = asyncio.create_task(
                make_openai_json_mode_ai_request(
                    client=OPENAI_CLIENT,
                    system_prompt=prompts_by_segment_by_language[language][segment_number],
                    llm_model=DEFAULT_LLM,
                    user_input=None,
                    prompt_model=MatchedTranslatedSegment,
                )
            )
            result_addresses.append(address)
            tasks.append(task)

    # Run all tasks concurrently


    logger.info(f"Running {len(tasks)} segment-level translation tasks concurrently")
    results = await asyncio.gather(*[task for task in tasks], return_exceptions=True)
    logger.info(f"Finished running {len(tasks)} segment-level translation tasks")
    # Handle the results and exceptions
    for result, address in zip(results, result_addresses):
        try:
            target_language = address['language']
            segment_number = address['segment_number']
            if translated_transcript_with_words.segments[segment_number].matched_translated_segment_by_language is None:
                translated_transcript_with_words.segments[segment_number].matched_translated_segment_by_language = {}
            translated_transcript_with_words.segments[segment_number].matched_translated_segment_by_language[target_language] = result
        except Exception as e:
            logger.error(f"Error processing {address}: {str(e)}")
            # Handle the exception

    return translated_transcript_with_words


async def full_text_and_segment_translation(og_transcription):
    # Full-text & segment level translation
    segment_level_system_prompt = format_full_segement_level_transcript_translation_system_prompt(
        initialized_translated_transcript_without_words=TranslatedTranscriptionWithoutWords.initialize(
            og_transcription=og_transcription))
    segment_level_translated_transcript = await make_openai_json_mode_ai_request(client=OPENAI_CLIENT,
                                                                                 system_prompt=segment_level_system_prompt,
                                                                                 llm_model=DEFAULT_LLM,
                                                                                 user_input=None,
                                                                                 prompt_model=TranslatedTranscriptionWithoutWords,
                                                                                 )
    logger.debug(f"Segment-level translation result: \n\n"
                 f"{segment_level_translated_transcript.model_dump_json(indent=2)}\n\n"
                 f"++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++\n\n")
    return segment_level_translated_transcript


def validate_translated_segment(segment_number, translated_segment, translated_transcript_with_words):
    if not len(translated_segment.words) == len(translated_transcript_with_words.segments[segment_number].words):
        raise ValueError(f"Word-level translation mismatch: "
                         f"Original segment has length {len(translated_transcript_with_words.segments[segment_number].words)}, "
                         f"translated segment has length {len(translated_segment.words)}")
    if not all([translated_segment.words[i].original_word ==
                translated_transcript_with_words.segments[segment_number].words[i].original_word for i in
                range(len(translated_segment.words))]):
        raise ValueError(f"Word-level translation mismatch: "
                         f"Original segment has words {[word.original_word for word in translated_transcript_with_words.segments[segment_number].words]}, "
                         f"translated segment has words {[word.original_word for word in translated_segment.words]}")
    if not translated_transcript_with_words.segments[
               segment_number].original_segment_text == translated_segment.original_segment_text:
        raise ValueError(f"Word-level translation mismatch: "
                         f"Original segment has text {translated_transcript_with_words.segments[segment_number].original_segment_text}, "
                         f"translated segment has text {translated_segment.original_segment_text}")
