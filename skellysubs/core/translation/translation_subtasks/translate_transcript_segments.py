import asyncio
import logging

from openai.types.audio import TranscriptionVerbose
from openai.types.audio.transcription_segment import TranscriptionSegment

from skellysubs.ai_clients.ai_client_strategy import get_ai_client
from skellysubs.core.translation.language_configs.language_configs import LanguageConfig
from skellysubs.core.translation.models.text_models import TranslatedTextModel
from skellysubs.core.translation.models.transcript_segment_models import TranslatedTranscriptSegment
from skellysubs.core.translation.models.translation_typehints import LanguageNameString

logger = logging.getLogger(__name__)

SEGMENT_LEVEL_TRANSCRIPT_TRANSLATION_SYSTEM_PROMPT = """

You are an expert translator. 

You will be given the transcription of an audio recording in {original_language} and asked to translate a section of it into {target_language_name}.: 

{target_language_config}


Your task is to provide a translation for a  single timestamped segment from the list of segments  that make up the full transcript. Your 
job is to translate the provided segment into the target language and provide the romanization method specified (if applicable). 


Remember, this is an audio transcription, so the text may contain errors. Please do your best to provide an accurate 
translation of the transcription and attempt to match the speaker's meaning and intention as closely as possible.

Here is the full transcript for context:
-------
FULL TRANSCRIPTION TEXT START: 

{full_transcription_text_in_original_language}

FULL TRANSCRIPTION TEXT END

And here is the full transcript translated into the target language:

FULL TRANSCRIPTION TEXT TRANSLATED INTO TARGET LANGUAGE START:

{full_transcription_text_in_target_language}

FULL TRANSCRIPTION TEXT TRANSLATED INTO TARGET LANGUAGE END

----

Here is the segment you should translate:

SECTION OF ORIGINAL TEXT TO TRANSLATE (Section# {segment_number} of {total_segments}):

{current_segment_in_original_language}

starting_timestamp: {start_timestamp}
ending_timestamp: {end_timestamp}
total_transcript_audio_duration: {duration}

END OF SECTION OF ORIGINAL TEXT TO TRANSLATE
-------

REMEMBER! Your task is to translate the text from the `SECTION OF ORIGINAL TEXT TO TRANSLATE` into the target language and provide the romanization method specified (if applicable).

If the target languages matches the original language, just return the original language transcript.

Your answer must match the form of the JSON schema provided. 

"""


def format_segment_prompts(
        original_full_text: str,
        translated_full_text: TranslatedTextModel,
        segments: list[TranscriptionSegment],
        target_language: LanguageConfig,
        original_language: LanguageNameString) -> list[str]:
    segment_level_prompts = []
    translated_text_str = translated_full_text.translated_text
    if translated_full_text.romanized_text:
        translated_text_str += f"ROMANIZED:\n\n{translated_full_text.romanized_text}"
    for segment_number, segment in enumerate(segments):
        segment_level_prompts.append(
            SEGMENT_LEVEL_TRANSCRIPT_TRANSLATION_SYSTEM_PROMPT.format(
                original_language=original_language,
                target_language_name=target_language.language_name,
                target_language_config=target_language.model_dump_json(indent=2),
                full_transcription_text_in_original_language=translated_text_str,
                full_transcription_text_in_target_language=translated_full_text.translated_text,
                segment_number=segment_number,
                total_segments=len(segments),
                current_segment_in_original_language=segment.text,
                start_timestamp=segment.start,
                end_timestamp=segment.end,
                duration=segments[-1].end,
            ))

    return segment_level_prompts


async def transcript_translation(original_transcript: TranscriptionVerbose,
                                 full_text_translations: dict[LanguageNameString, TranslatedTextModel],
                                 target_languages: dict[LanguageNameString, LanguageConfig],
                                 ) -> tuple[
    dict[LanguageNameString, list[str]], dict[LanguageNameString, list[TranslatedTranscriptSegment]]]:
    tasks = []
    addresses = []
    segment_prompts_by_language: dict[LanguageNameString, list[str]] = {}
    for language, translation in full_text_translations.items():
        segment_prompts_by_language[language] = format_segment_prompts(
            target_language=target_languages[language],
            original_language=original_transcript.language,
            original_full_text=original_transcript.text,
            translated_full_text=translation,
            segments=original_transcript.segments,
        )
        for index, segment_system_prompt in enumerate(segment_prompts_by_language[language]):
            addresses.append({'language': language, 'index': index})
            tasks.append(asyncio.create_task(get_ai_client().make_json_mode_request(
                system_prompt=segment_system_prompt,
                prompt_model=TranslatedTranscriptSegment,
            )))

    logger.info(f"Running {len(tasks)} segment-level translation tasks concurrently")
    results = await asyncio.gather(*tasks, return_exceptions=True)
    logger.info(f"Finished running {len(tasks)} segment-level translation tasks")

    translated_segments_by_language = {}
    for result, address in zip(results, addresses):
        target_language = address['language']
        index = address['index']

        if isinstance(result, Exception):
            logger.error(f"Error in task {address}: {str(result)}")
            raise ValueError(f"Segment translation failed for {target_language} segment {index}") from result

        if not isinstance(result, TranslatedTranscriptSegment):
            logger.error(f"Invalid response type for {address}: {type(result)}")
            raise TypeError(f"Expected TranslatedTranscriptSegment, got {type(result)}")

        if target_language not in translated_segments_by_language:
            translated_segments_by_language[target_language] = []

        try:
            translated_segments_by_language[target_language].append(result)
            if len(translated_segments_by_language[target_language]) != index + 1:
                raise ValueError(f"Index mismatch in {target_language} at index {index}")
        except Exception as e:
            logger.error(f"Error processing {address}: {str(e)}")
            raise

    return segment_prompts_by_language, translated_segments_by_language
