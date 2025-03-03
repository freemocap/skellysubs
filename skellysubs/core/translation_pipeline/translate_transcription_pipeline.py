import asyncio
import logging

from skellysubs.ai_clients.ai_client_strategy import get_ai_client
from skellysubs.core.audio_transcription.whisper_transcript_result_model import WhisperTranscriptionResult
from skellysubs.core.translation_pipeline.language_configs.language_configs import LanguageConfig
from skellysubs.core.translation_pipeline.models.translated_segment_models import MatchedTranslatedSegment
from skellysubs.core.translation_pipeline.models.translated_text_models import TranslatedText
from skellysubs.core.translation_pipeline.models.translated_transcript_model import TranslatedTranscription
from skellysubs.core.translation_pipeline.models.translation_typehints import LanguageNameString
from skellysubs.core.translation_pipeline.translation_prompts.full_text_transcript_translation_prompt import \
    format_full_text_translation_system_prompt
from skellysubs.core.translation_pipeline.translation_prompts.segement_word_level_translation_prompt import \
    format_segment_word_level_transcript_translation_system_prompts
from skellysubs.core.translation_pipeline.translation_prompts.segment_level_transcript_translation_prompt import \
    format_segment_level_translation_system_prompts

logger = logging.getLogger(__name__)


async def translate_transcription_pipeline(og_transcription: WhisperTranscriptionResult,
                                           original_language: str = "ENGLISH"
                                           ) -> TranslatedTranscription:
    initialized_transcription = TranslatedTranscription.initialize(og_transcription=og_transcription,
                                                                   original_langauge=original_language)
    full_text_translated_transcript = await full_text_translation(initialized_transcription=initialized_transcription)

    segment_level_translated_transcript = await segment_level_translation(
        full_text_translated_transcript=full_text_translated_transcript)

    translated_transcript_with_words = await word_level_translation_and_matching(
        segment_level_translated_transcript=segment_level_translated_transcript)

    return translated_transcript_with_words


async def word_level_translation_and_matching(segment_level_translated_transcript: TranslatedTranscription):
    # Word-level translation

    prompts_by_segment_by_language = format_segment_word_level_transcript_translation_system_prompts(
        initialized_translated_transcript=segment_level_translated_transcript)

    tasks = []
    result_addresses = []
    for language in segment_level_translated_transcript.translations.translations.keys():
        for segment_number, segment in enumerate(segment_level_translated_transcript.segments):
            address = {'language': language, 'segment_number': segment_number}
            task = asyncio.create_task(get_ai_client().make_json_mode_request(
                system_prompt=prompts_by_segment_by_language[language][segment_number],
                prompt_model=MatchedTranslatedSegment,  # type: ignore
            )
            )
            result_addresses.append(address)
            tasks.append(task)

    # Run all tasks concurrently

    logger.info(f"Running {len(tasks)} word-level translation tasks concurrently")
    results = await asyncio.gather(*[task for task in tasks], return_exceptions=True)
    logger.info(f"Finished running {len(tasks)} word-level translation tasks")
    # Handle the results and exceptions
    for result, address in zip(results, result_addresses):
        if not isinstance(result, MatchedTranslatedSegment):
            logger.error(f"Error in task {address}: {result}")
            raise ValueError(f"Error in task {address}: {result}")
        try:
            target_language = address['language']
            segment_number = address['segment_number']
            segment_level_translated_transcript.segments[segment_number].matched_translated_segment_by_language[
                target_language] = result
        except Exception as e:
            logger.error(f"Error processing {address}: {str(e)}")
            raise

    return segment_level_translated_transcript


async def segment_level_translation(
        full_text_translated_transcript: TranslatedTranscription) -> TranslatedTranscription:
    segment_level_system_prompts = format_segment_level_translation_system_prompts(
        full_text_translated_transcript=full_text_translated_transcript)

    tasks = []
    addresses = []
    for language, system_prompts in segment_level_system_prompts.items():
        for index, prompt in enumerate(system_prompts):
            addresses.append({'language': language, 'index': index})
            tasks.append(asyncio.create_task(get_ai_client().make_json_mode_request(system_prompt=prompt,
                                                                                    prompt_model=TranslatedText,
                                                                                    )
                                             )
                         )
    logger.info(f"Running {len(tasks)} segment-level translation tasks concurrently")
    results = await asyncio.gather(*[task for task in tasks], return_exceptions=True)
    logger.info(f"Finished running {len(tasks)} segment-level translation tasks")
    for result, address in zip(results, addresses):
        try:
            target_language = address['language']
            index = address['index']
            full_text_translated_transcript.segments[index].translations.translations[target_language] = result
        except Exception as e:
            logger.error(f"Error processing {address}: {str(e)}")
            raise

    return full_text_translated_transcript


async def full_text_translation(text: str, original_language: str,
                                target_languages: dict[LanguageNameString, LanguageConfig]) -> tuple[dict[LanguageNameString,str], dict[LanguageNameString, TranslatedText]]:
    # Full-text translation

    system_prompts_by_language = format_full_text_translation_system_prompt(text=text,
                                                                                      target_languages=target_languages,
                                                                                      original_language=original_language)

    full_text_tasks = []
    for language, system_prompt in system_prompts_by_language.items():
        full_text_tasks.append(asyncio.create_task(get_ai_client().make_json_mode_request(system_prompt=system_prompt,
                                                                                          prompt_model=TranslatedText,
                                                                                          )))

    translations = {}
    results: list[TranslatedText] = await asyncio.gather(*[task for task in full_text_tasks], return_exceptions=True)
    for key, result in zip(target_languages.keys(), results):
        translations[key] = result

    return system_prompts_by_language, translations


