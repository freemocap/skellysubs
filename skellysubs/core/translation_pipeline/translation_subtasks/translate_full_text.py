import asyncio
import logging

from skellysubs.ai_clients.ai_client_strategy import get_ai_client
from skellysubs.core.translation_pipeline.language_configs.language_configs import LanguageConfig
from skellysubs.core.translation_pipeline.models.translated_text_models import TranslatedText
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

async def text_translation(text: str, original_language: str,
                           target_languages: dict[LanguageNameString, LanguageConfig]) -> tuple[
    dict[LanguageNameString, str], dict[LanguageNameString, TranslatedText]]:
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
