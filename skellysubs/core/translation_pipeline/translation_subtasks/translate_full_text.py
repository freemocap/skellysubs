import asyncio
import json
import logging
import re

from skellysubs.ai_clients.ai_client_strategy import get_ai_client
from skellysubs.core.translation_pipeline.language_configs.language_configs import LanguageConfig
from skellysubs.core.translation_pipeline.models.translated_text import TranslatedText
from skellysubs.core.translation_pipeline.models.translation_typehints import LanguageNameString

from skellysubs.utilities.get_wikipedia_article_contents import get_wikipedia_texts

logger = logging.getLogger(__name__)


# Here is some information about the language you will be translating into:
#
# BEGIN TARGET LANGUAGE INFO:
#
# {target_language_info}
#
# END TARGET LANGUAGE INFO


FULL_TEXT_TRANSLATION_SYSTEM_PROMPT = """
Return your answer in the format specified by the provided JSON schema.

You are an expert translator. 

You will be some text in  {original_language} and asked to translate it into this language. :  

BEGIN TARGET LANGUAGE CONFIG:
{target_language_config}
END TARGET LANGUAGE CONFIG


-------
You will be provided with a transcript of the original language, along with a single section from the full text. 
Your task is to translate the separated section of text into the target language and provide the romanization method specified (if applicable).



-------
Here is the full text in the original language: 
FULL ORIGINAL LANGUAGE TEXT:

'''
{original_text}
'''
FULL END OF ORIGINAL LANGUAGE TEXT

--------

And here is the section you should translate:
SECTION OF ORIGINAL TEXT TO TRANSLATE (Section# {segment_number} of {total_segments}):
'''
{current_segment}
'''
END OF SECTION OF ORIGINAL TEXT TO TRANSLATE

Remember your task is to translate ONLY the PROVIDED SECTION of text provided into the target language and provide the romanization method specified (if applicable).
-------

Return your answer in the format specified by the provided JSON schema.
"""


async def format_full_text_translation_system_prompt(
        text: str,
        original_language: str,
        target_languages: dict[LanguageNameString, LanguageConfig],
        max_length=100) -> dict[str, list[str]]:
    full_text_translation_prompts_by_language = {}
    text_segments = split_text_into_segments(text=text, max_word_length=max_length)
    total_segments = len(text_segments)

    for language, config in target_languages.items():

        # wikipedia_link_info = json.dumps({link: await get_wikipedia_texts(config.background.wikipedia_links) for link in
        #                                   config.background.wikipedia_links}, indent=2)

        language_prompts = []
        for segment_number, segment in enumerate(text_segments):
            prompt = FULL_TEXT_TRANSLATION_SYSTEM_PROMPT.format(
                original_language=original_language,
                target_language_config=config.model_dump_json(indent=2),
                original_text=text,
                current_segment=segment,
                segment_number=segment_number + 1,
                total_segments=total_segments
            )
            language_prompts.append(prompt)

        full_text_translation_prompts_by_language[language] = language_prompts

    logger.debug(
        "==================================\n\n"
        f"Formatted system prompts: \n\n"
        f" FULL TEXT/SEGMENT LEVEL SYSTEM PROMPT: \n"
        f"{full_text_translation_prompts_by_language}\n\n"
        "==================================\n\n"
    )

    return full_text_translation_prompts_by_language

def split_text_into_segments(text, max_word_length=150):
    words = text.split()
    segments = []
    current_segment = []

    for word in words:
        current_segment.append(word)
        if len(current_segment) >= max_word_length:
            segments.append(' '.join(current_segment))
            current_segment = []

    if current_segment:
        segments.append(' '.join(current_segment))

    return segments
async def text_translation(text: str, original_language: str,
                           target_languages: dict[LanguageNameString, LanguageConfig]) -> tuple[
    dict[LanguageNameString, list[str]], dict[LanguageNameString, TranslatedText]]:

    # Full-text translation
    system_prompts_by_language = await format_full_text_translation_system_prompt(
        text=text,
        target_languages=target_languages,
        original_language=original_language
    )

    # Create a list to hold all tasks
    all_translation_tasks = []
    language_addresses = []

    for language, system_prompts in system_prompts_by_language.items():


        for prompt in system_prompts:
            task = asyncio.create_task(
                get_ai_client().make_json_mode_request(
                    system_prompt=prompt,
                    prompt_model=TranslatedText,
                )
            )
            all_translation_tasks.append(task)
            language_addresses.append(language)

    # Run all tasks concurrently
    logger.info(f"Running {len(all_translation_tasks)} full-text translation tasks concurrently")
    results = await asyncio.gather(*all_translation_tasks, return_exceptions=True)
    logger.info("Finished running all full-text translation tasks")

    # Organize results by language
    translations = {}
    results_by_language: dict[LanguageNameString, list[TranslatedText]] = {}
    for language, translated_segment in zip(language_addresses, results):
        if language not in results_by_language:
            results_by_language[language] = []
        results_by_language[language].append(translated_segment)

    for language, translated_segments in results_by_language.items():
        # Reassemble the translated segments into the full text
        translated_full_text = ' '.join([segment.translated_text for segment in translated_segments])
        romanized_full_text = ' '.join([segment.romanized_text for segment in translated_segments])
        translations[language] = TranslatedText(
            **translated_segments[0].model_dump(exclude={'translated_text', 'romanized_text'}),
            translated_text=translated_full_text,
            romanized_text=romanized_full_text
        )

    return system_prompts_by_language, translations
