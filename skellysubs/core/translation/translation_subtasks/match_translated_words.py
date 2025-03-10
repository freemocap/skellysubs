import asyncio
import logging

from skellysubs.ai_clients.ai_client_strategy import get_ai_client
from skellysubs.core.translation.language_configs.language_configs import get_language_configs
from skellysubs.core.translation.models.old_transcript_models import MatchedTranslatedSegment
from skellysubs.core.translation.models.translated_transcript_model import \
    OldTranslatedTranscription

logger = logging.getLogger(__name__)
WORD_LEVEL_TRANSLATION_MATCHING_TASK_INSTRUCTIONS = """
You are an expert translator. 

You will be given the result of a Whisper transcription of an audio recording in {original_language} which has already
 been translated into the following language:  

{target_language_with_their_romanization_methods}

You will be shown a segment of the original language transcript with the words are indexed, so e.g. "Hi my name is Jon" 
becomes "([0]Hi) ([1]my) ([2]name) ([3]is) ([4]Jon)"

You will then be provided with an indexed list of words from the target language (where the index corresponds to the 
position of the target-language word in the translated segement).

Your task is to match the word from the original language with the closest matching word from the target languages. 
If you cannot find a matching word, do your best!

If a word in the target language covers multiple words in the original language (such as 'hablo' in Spanish encompassing
 both 'I' and 'speak' in English), you can match the same word multiple times. 

### EXAMPLE OF A TIME WHEN THE TARGET LANGUAGE WORD COVERS MULTIPLE WORDS IN THE ORIGINAL LANGUAGE ###

Example original language segment: "My Name is Jon"
([0] Hello, [starting_timestamp: 0.0, ending_timestamp: 0.42])
([1] my [starting_timestamp: 0.46, ending_timestamp: 0.58])
([2] name [starting_timestamp: 0.58, ending_timestamp: 0.78])
([3] is [starting_timestamp: 0.78, ending_timestamp: 0.96])
([4] Jon [starting_timestamp: 0.96, ending_timestamp: 1.14])

Example target language word list in Arabic:
([0]مرحبًا)
([1]اسمي)
([2]جون)

Note that the Arabic word 'اسمي' covers all of the words in `my name is` in English, so you would match each of the 
English words 'I', 'name', and 'is' with the Arabic word 'اسمي' in this case.

So the correct matching would be:
hello: مرحبًا 
my: اسمي
name: اسمي
is: اسمي
Jon: جون

### END OF EXAMPLE ###


### BEGINNING OF THE TEXT DATA YOU WILL BE WORKING WITH ###

Here is segment of the original language transcript (including each words index in the segment and the start/end timestamp of when the word was spoken in the original audio): 

{current_segment_in_original_language_including_indexed_words_and_timestamps}


Here is the indexed list of words from the target language

{indexed_list_of_available_words_in_target_languages}

### END OF TEXT DATA ###

Your job is to find find the best matching word from the indexed list of translated words for each word in the original 
language segment.

if a word is repeated in the segment, attempt to match the word in the target language who's index is closest to the 
original word's index (e.g. If the word 'you' is used at the beginning of the segement and then again at the end,
    try to match the first 'you' with the first 'you' in the target language list, and the second 'you' with the second
    'you' in the target language list)

The idea is that we're going to be putting these on a video as subtitles, and when the speaker says the word in the
 original language, we want the subtitle to show the word in the target language that best matches the original word. 
Obviously this won't be perfectly one-to-one because of the complexities of language, but do your best to match the
 words as closely as possible with the intention of finding matches that make the most linguistic sense and would help 
 a person who speaks the target language understand the original language and vice versa.

If the target languages matches the original language, just return the original language transcript.

Your answer must be provided in accordance to the JSON format provided in the prompt.

"""


def format_segment_word_level_transcript_translation_system_prompts(
        initialized_translated_transcript: OldTranslatedTranscription) -> dict[str, dict[int, str]]:
    logger.debug(f"Word-level translation prompts for {initialized_translated_transcript.original_language} \n\n")
    prompts_by_segment_by_language = {}

    for language in initialized_translated_transcript.og_text_and_translations.keys():
        # if "original" in language.lower():
        #     continue
        prompts_by_segment_by_language[language] = {}
        for segment_number, segment in enumerate(initialized_translated_transcript.segments):
            prompts_by_segment_by_language[language][
                segment_number] = WORD_LEVEL_TRANSLATION_MATCHING_TASK_INSTRUCTIONS.format(
                original_language=initialized_translated_transcript.original_language,
                target_language_with_their_romanization_methods=get_language_configs()[
                    language.lower()].model_dump_json(indent=2, exclude={"annotation_config"}),
                current_segment_in_original_language_including_indexed_words_and_timestamps=segment.original_segment_text_with_words_indexed_and_timestamped,
                indexed_list_of_available_words_in_target_languages="\n".join(
                    segment.get_indexed_word_list_by_language(language))
            )

    return prompts_by_segment_by_language


async def word_level_translation_and_matching(segment_level_translated_transcript: OldTranslatedTranscription):
    # Word-level translation

    prompts_by_segment_by_language = format_segment_word_level_transcript_translation_system_prompts(
        initialized_translated_transcript=segment_level_translated_transcript)

    tasks = []
    result_addresses = []
    for language in segment_level_translated_transcript.full_text_translations.translations.keys():
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
