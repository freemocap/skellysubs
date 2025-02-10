import logging

from skellysubs.skellysubs_core.translation_pipeline.models.translated_transcript_model import \
    TranslatedTranscription
from skellysubs.skellysubs_core.translation_pipeline.models.translation_typehints import LanguageNameString

logger = logging.getLogger(__name__)


SEGMENT_TRANSLATION_INSTRUCTIONS = """
You will be given the result of a Whisper transcription of an audio recording in {original_language}, and asked to 
provide a translation of the full text and a single timestamped segment from the list of segments  that make up the full transcript. Your 
job is to translate the match the portion of the translated full text that matches the provided original language segment. 

You should be trying to match the transcript by timing, so that if the translated section you specified were shown on 
a video at the same time as the original segment, the translated text would match the original segment and a speaker
of the translated language would be able to understand the video.

Make sure that all requested languages are translated accurately and that any romanizations are correct. Make sure that 
all languages cover the full meaning of the original transcribed text. 


Remember, this is an audio transcription, so the text may contain errors. Please do your best to provide an accurate 
translation of the transcription and attempt to match the speaker's meaning and intention as closely as possible.

"""


SEGMENT_LEVEL_TRANSCRIPT_TRANSLATION_SYSTEM_PROMPT = """

You are an expert translator. 

You will be given the result of a Whisper transcription of an audio recording in {original_language} which has already
 been translated into the following language:  

{target_language_with_their_romanization_methods}


Your task is: 

{translation_task_instructions}

-------
FULL TRANSCRIPTION TEXT START: 

{full_transcription_text_in_original_language}

FULL TRANSCRIPTION TEXT END

----
SECTION OF ORIGINAL TEXT TO TRANSLATE (Section# {segment_number} of {total_segments}):

{current_segment_in_original_language}

starting_timestamp: {start_timestamp}
ending_timestamp: {end_timestamp}
total_transcript_audio_duration: {duration}

END OF SECTION OF ORIGINAL TEXT TO TRANSLATE
-------

REMEMBER! Your task is: 

{translation_task_instructions_repeated}

Your answer must match the form of the JSON schema provided. 

"""


def format_segment_level_translation_system_prompts(
        full_text_translated_transcript: TranslatedTranscription) -> dict[LanguageNameString, list[str]]:
    segment_level_prompts_by_language = {}
    for segment_number, segment in enumerate(full_text_translated_transcript.segments):
        for language_name, language_pair in full_text_translated_transcript.target_laguage_pairs.items():
            if language_name not in segment_level_prompts_by_language:
                segment_level_prompts_by_language[language_name] = []
            segment_level_prompts_by_language[language_name].append(SEGMENT_LEVEL_TRANSCRIPT_TRANSLATION_SYSTEM_PROMPT.format(
                original_language=full_text_translated_transcript.original_language,
                target_language_with_their_romanization_methods=language_pair.model_dump_json(indent=2),
                translation_task_instructions=SEGMENT_TRANSLATION_INSTRUCTIONS,
                full_transcription_text_in_original_language=full_text_translated_transcript.original_text,
                segment_number=segment_number,
                total_segments=len(full_text_translated_transcript.segments),
                current_segment_in_original_language=segment.original_segment_text,
                start_timestamp=segment.start,
                end_timestamp=segment.end,
                duration=full_text_translated_transcript.segments[-1].end,
                translation_task_instructions_repeated=SEGMENT_TRANSLATION_INSTRUCTIONS,
            ))

    logger.debug(
        "==================================\n\n"
        f"Formatted system prompts: \n\n"
        f" FULL TEXT/SEGMENT LEVEL SYSTEM PROMPT: \n"
        f"{segment_level_prompts_by_language}\n"
        "==================================\n\n"
    )

    return segment_level_prompts_by_language

