import asyncio
import logging

from skellysubs.skellysubs_core.translation_pipeline.translation_prompts.full_text_transcript_translation_prompt import \
    format_full_text_translation_system_prompt
from skellysubs.skellysubs_core.translation_pipeline.translation_prompts.segement_word_level_translation_prompt import \
    format_segment_word_level_transcript_translation_system_prompts

from skellysubs.ai_clients.ai_client_strategy import get_ai_client

from skellysubs.skellysubs_core.audio_transcription.whisper_transcript_result_model import WhisperTranscriptionResult
from skellysubs.skellysubs_core.translation_pipeline.models.language_models import LanguageNames
from skellysubs.skellysubs_core.translation_pipeline.models.translated_transcript_model import TranslatedTranscription
from skellysubs.skellysubs_core.translation_pipeline.models.translated_segment_models import MatchedTranslatedSegment
from skellysubs.skellysubs_core.translation_pipeline.models.translated_text_models import TranslatedText
from skellysubs.skellysubs_core.translation_pipeline.translation_prompts.segment_level_transcript_translation_prompt import \
    format_segment_level_translation_system_prompts

logger = logging.getLogger(__name__)



async def translate_transcription_pipeline(og_transcription: WhisperTranscriptionResult
                                           ) -> TranslatedTranscription:
    initialized_transcription = TranslatedTranscription.initialize(og_transcription=og_transcription)
    full_text_translated_transcript = await full_text_translation(initialized_transcription=initialized_transcription)

    segment_level_translated_transcript = await segment_level_translation(
        full_text_translated_transcript=full_text_translated_transcript)

    translated_transcript_with_words = await word_level_translation_and_matching(
        segment_level_translated_transcript=segment_level_translated_transcript)

    return translated_transcript_with_words


async def word_level_translation_and_matching(segment_level_translated_transcript:TranslatedTranscription):
    # Word-level translation

    prompts_by_segment_by_language = format_segment_word_level_transcript_translation_system_prompts(
        initialized_translated_transcript=segment_level_translated_transcript)

    tasks = []
    result_addresses = []
    for language in segment_level_translated_transcript.og_text_and_translations.keys():
        for segment_number, segment in enumerate(segment_level_translated_transcript.segments):
            address = {'language': language, 'segment_number': segment_number}
            task = asyncio.create_task(get_ai_client().make_json_mode_request(
                    system_prompt=prompts_by_segment_by_language[language][segment_number],
                    prompt_model=MatchedTranslatedSegment, # type: ignore
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
            #TODO - skip English and just copy the original text into the relevant whosit
            full_text_translated_transcript.segments[index].set_translation_by_language(language=target_language,
                                                                                        translation=result)
        except Exception as e:
            logger.error(f"Error processing {address}: {str(e)}")
            raise

    return full_text_translated_transcript


async def full_text_translation(initialized_transcription:TranslatedTranscription) -> TranslatedTranscription:
    # Full-text translation

    full_text_system_prompts_by_language = format_full_text_translation_system_prompt(
        initialized_translated_transcript_without_words=initialized_transcription)

    full_text_tasks = []
    results = []
    for language, system_prompt in full_text_system_prompts_by_language.items():
        # full_text_tasks.append(asyncio.create_task(make_openai_json_mode_ai_request(client=OPENAI_CLIENT,
        #                                                                             system_prompt=system_prompt,
        #                                                                             llm_model=DEFAULT_LLM,
        #                                                                             user_input=None,
        #                                                                             prompt_model=TranslatedText,
        #                                                                             )))
        results.append(await get_ai_client().make_json_mode_request(system_prompt=system_prompt,
                                         prompt_model=TranslatedText, # type: ignore
                                         ))
    # results: list[TranslatedText] = await asyncio.gather(*[task for task in full_text_tasks], return_exceptions=True)
    for result in results:
        if result.translated_language.lower() in LanguageNames.SPANISH.value.lower():
            initialized_transcription.translations.spanish = result
        elif result.translated_language.lower() in LanguageNames.ARABIC_LEVANTINE.value.lower():
            initialized_transcription.translations.arabic = result
        elif result.translated_language.lower() in LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
            initialized_transcription.translations.chinese = result
        elif result.translated_language.lower() in LanguageNames.HINDI.value.lower():
            initialized_transcription.translations.hindi = result
        elif result.translated_language.lower() in LanguageNames.ENGLISH.value.lower():
            initialized_transcription.translations.english = result

        else:
            raise ValueError(f"Unrecognized language: {result.translated_language}")

    return initialized_transcription


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
