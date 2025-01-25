from skellysubs.add_subtitles_to_video_pipeline.full_text_transcript_translation_prompt import \
    BASE_TRANSLATION_PROMPT
from skellysubs.translate_transcript_pipeline.models.language_models import LanguageNames
from skellysubs.translate_transcript_pipeline.models.translated_transcript_model import \
    TranslatedTranscription

import logging
logger = logging.getLogger(__name__)
SEGMENT_WORD_LEVEL_TASK_INSTRUCTIONS = """
You are an expert translator. 

You will be given the result of a Whisper transcription of an audio recording in {original_language} which has already been translated into the following language:  

{target_language_with_their_romanization_methods}

You will be shown a segment of the original language transcript and a particular word from that segment that we are matching right now.  

You will then be provided with an indexed list of words from the target languages.

Your task is to match the word from the original language with the closest matching word from the target languages. If you cannot find a matching word, do your best!

If a word in the target language covers multiple words in the original language (such as 'hablo' in Spanish encompassing both 'I' and 'speak' in English), you can use the same word multiple times
    

Here is segment of the original language transcript. Note that the words are indexed, so e.g. "Hi my name is Jon" becomes "([0]Hi) ([1]my) ([2]name) ([3]is) ([4]Jon)"- if a word is repeated in the segment, attempt to match the word in the target language who's index is closest to the original word's index:

{current_segment_in_original_language_including_indexed_words}

Here is the word from that segment that we are matching right now:

{current_word_in_original_language_indexed}

Here is the indexed list of words from the target language

{indexed_list_of_available_words_in_target_languages}

Select the word from the indexed list above that best matches the word from the original language. If you cannot find a matching word, do your best!
If a word in the target language covers multiple words in the original language (such as 'hablo' in Spanish encompassing both 'I' and 'speak' in English)

Your answer must be provided in accordance to the JSON format provided in the prompt.

"""

def format_segment_word_level_transcript_translation_system_prompts(
        initialized_translated_transcript: TranslatedTranscription) -> dict[str, dict[int, dict[int, str]]]:

    logger.debug(f"Word-level translation prompts for {initialized_translated_transcript.original_language} \n\n")
    prompt_by_word_by_segment_by_language = {}

    for language in initialized_translated_transcript.og_text_and_translations.keys():
        prompt_by_word_by_segment_by_language[language] = {}
        for segment_number, segment in enumerate(initialized_translated_transcript.segments):
            prompt_by_word_by_segment_by_language[language][segment_number] = {}
            for word_number, word in enumerate(segment.get_word_list_by_language(LanguageNames.ENGLISH.value)):
                prompt_by_word_by_segment_by_language[language][segment_number][word_number] = SEGMENT_WORD_LEVEL_TASK_INSTRUCTIONS.format(
                    original_language=initialized_translated_transcript.original_language,
                    target_language_with_their_romanization_methods=initialized_translated_transcript.language_pair_by_language(language).model_dump_json(indent=2),
                    current_segment_in_original_language_including_indexed_words=segment.original_segment_text_with_words_indexed,
                    current_word_in_original_language_indexed = f"([{word_number}]{word})",
                    indexed_list_of_available_words_in_target_languages="\n".join(segment.get_indexed_word_list_by_language(language))
                )

    return prompt_by_word_by_segment_by_language
