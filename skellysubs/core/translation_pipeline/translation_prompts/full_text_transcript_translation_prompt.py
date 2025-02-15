import logging



from skellysubs.core.translation_pipeline.language_configs.language_configs import get_language_configs
from skellysubs.core.translation_pipeline.models.translated_transcript_model import TranslatedTranscription

logger = logging.getLogger(__name__)




FULL_TEXT_TRANSLATION_SYSTEM_PROMPT = """

You are an expert translator. 

You will be given the result of a Whisper transcription of an audio recording in {original_language} which has already
 been translated into the following language:  

{target_language_with_their_romanization_methods}

You will be provided with a transcript of the original language, and your task is to translate the text into the target
language and provide the romanization method specified (if applicable).

Remember, this is an audio transcription, so the text may contain errors. Please do your best to provide an accurate 
translation of the transcription and attempt to match the speaker's meaning and intention as closely as possible.


-------
ORIGINAL LANGUAGE TRANSCRIPT:

'''
{original_language_transcript}
'''
END OF ORIGINAL LANGUAGE TRANSCRIPT


-------

REMEMBER! Your task is to translate the text from the ORIGINAL LANGUAGE TRANSCRIPT into the target
language and provide the romanization method specified (if applicable).

Remember, this is an audio transcription, so the text may contain errors. Please do your best to provide an accurate 
translation of the transcription and attempt to match the speaker's meaning and intention as closely as possible.

If the target languages matches the original language, just return the original language transcript.
"""


def format_full_text_translation_system_prompt(
        initialized_translated_transcript_without_words: TranslatedTranscription) -> dict[str, str]:
    full_text_translation_prompts_by_language = {}
    for language in initialized_translated_transcript_without_words.translated_languages:
        # if "english" in language.lower():
        #     continue
        full_text_translation_prompts_by_language[language] = FULL_TEXT_TRANSLATION_SYSTEM_PROMPT.format(
            original_language=initialized_translated_transcript_without_words.original_language,
            target_language_with_their_romanization_methods=get_language_configs()[language].model_dump_json(indent=2, exclude={"annotation_config"}),
            original_language_transcript=initialized_translated_transcript_without_words.original_text
        )

    logger.debug(
        "==================================\n\n"
        f"Formatted system prompts: \n\n"
        f" FULL TEXT/SEGMENT LEVEL SYSTEM PROMPT: \n"
        f"{full_text_translation_prompts_by_language}\n\n"
        "==================================\n\n"
    )

    return full_text_translation_prompts_by_language

