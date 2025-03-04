import logging



from skellysubs.core.translation_pipeline.language_configs.language_configs import get_language_configs, LanguageConfig
from skellysubs.core.translation_pipeline.models.translated_transcript_model import TranslatedTranscription
from skellysubs.core.translation_pipeline.models.translation_typehints import LanguageNameString

logger = logging.getLogger(__name__)




FULL_TEXT_TRANSLATION_SYSTEM_PROMPT = """

You are an expert translator. 

You will be some text in  {original_language} and asked to translate it into this language. :  

{target_language_config}

You will be provided with a transcript of the original language, and your task is to translate the text into the target
language and provide the romanization method specified (if applicable).


-------
ORIGINAL LANGUAGE TEXT:

'''
{original_text}
'''
END OF ORIGINAL LANGUAGE TEXT


-------

REMEMBER! Your task is to translate the text from the ORIGINAL LANGUAGE TEXT into the target
language and provide the romanization method specified (if applicable).

If the target languages matches the original language, just return the original language transcript.
"""


def format_full_text_translation_system_prompt(
        text:str,
        original_language: str,
        target_languages:dict[LanguageNameString, LanguageConfig]) -> dict[str, str]:
    full_text_translation_prompts_by_language = {}
    for language, config in target_languages.items():
        if original_language.lower() in language.lower():
            continue
        full_text_translation_prompts_by_language[language] = FULL_TEXT_TRANSLATION_SYSTEM_PROMPT.format(
            original_language=original_language,
            target_language_config=config.model_dump_json(indent=2),
            original_text=text
        )

    logger.debug(
        "==================================\n\n"
        f"Formatted system prompts: \n\n"
        f" FULL TEXT/SEGMENT LEVEL SYSTEM PROMPT: \n"
        f"{full_text_translation_prompts_by_language}\n\n"
        "==================================\n\n"
    )

    return full_text_translation_prompts_by_language

