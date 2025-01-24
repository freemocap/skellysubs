import json
import logging

from skellysubs.translate_transcript_pipeline.translated_transcript_model import TranslatedTranscriptionWithoutWords


logger = logging.getLogger(__name__)

BASE_TRANSLATION_PROMPT = """
You are an expert translator. You are trained in audio transcription and translation, and have been trained in the proper way to romanize languages that do not use the Latin alphabet (such as Chinese or Arabic).

You are fluent and trained in the following languages (and their romanization methods): 

{languages}

"""

SEGMENT_TRANSLATION_INSTRUCTIONS = """
You will be given the result of a Whisper transcription of an audio recording in {original_language}, and asked to provide a translation of the full text and a list of the timestamped segments that make up the full transcript. Your job is to translate the original text into each of the target lanauages defined in the initialized TranslatedTranscription object.

You will be provided a JSON schema and a partially initialized TranslatedTranscription object that contains the original text and the target languages you are expected to translate the text into (including any romanization requirements).

You should begin by translating the entire text, and then break it up into segments to match the original transcription (Keep the original timestamps!)

Make sure that all requested languages are translated accurately and that any romanizations are correct. Make sure that all languages cover the full meaning of the original transcribed text. 

Remember, this is an audio transcription, so the text may contain errors. Please do your best to provide an accurate translation of the transcription and attempt to match the speaker's meaning and intention as closely as possible.

Here is the initialized TranslatedTranscription object that you will be working with, which contains the original text and the target languages you are expected to translate the text into (including any romanization requirements) - Fill in the sections that say "NOT YET TRANSLATED" with your translations/romanizations:
"""


TRANSCRIPT_TRANSLATION_SYSTEM_PROMPT = """

{base_translation_prompt}

Your task is: 

{translation_task_instructions}

-------
{initialized_translated_transcription_object}
-------

REMEMBER! Your task is: 

{translation_task_instructions_repeated}

"""


def format_full_segement_level_transcript_translation_system_prompt(
        initialized_translated_transcript_without_words: TranslatedTranscriptionWithoutWords) -> str:
    full_text_segment_level_prompt = TRANSCRIPT_TRANSLATION_SYSTEM_PROMPT.format(
        base_translation_prompt=BASE_TRANSLATION_PROMPT,
        translation_task_instructions=SEGMENT_TRANSLATION_INSTRUCTIONS,
        languages=initialized_translated_transcript_without_words.target_languages_as_string,
        original_language=initialized_translated_transcript_without_words.original_language,
        initialized_translated_transcription_object=json.dumps(initialized_translated_transcript_without_words.model_dump(), indent=2),
        translation_task_instructions_repeated=SEGMENT_TRANSLATION_INSTRUCTIONS,
    )

    logger.debug(
        "==================================\n\n"
        f"Formatted system prompts: \n\n"
        f" FULL TEXT/SEGMENT LEVEL SYSTEM PROMPT: \n"
        f"{full_text_segment_level_prompt}\n\n"
        "==================================\n\n"
    )

    return full_text_segment_level_prompt

