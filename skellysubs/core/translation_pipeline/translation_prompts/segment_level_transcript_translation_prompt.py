import logging

from skellysubs.core.translation_pipeline.language_configs.language_configs import get_language_configs
from skellysubs.core.translation_pipeline.models.translated_transcript_model import \
    TranslatedTranscription
from skellysubs.core.translation_pipeline.models.translation_typehints import LanguageNameString

logger = logging.getLogger(__name__)



SEGMENT_LEVEL_TRANSCRIPT_TRANSLATION_SYSTEM_PROMPT = """

You are an expert translator. 

You will be given the result of a Whisper transcription of an audio recording in {original_language} and asked to translate a section of it into the following language:

{target_language_with_their_romanization_methods}


Your task is to provide a translation for a  single timestamped segment from the list of segments  that make up the full transcript. Your 
job is to translate the provided segment into the target language and provide the romanization method specified (if applicable). 


Remember, this is an audio transcription, so the text may contain errors. Please do your best to provide an accurate 
translation of the transcription and attempt to match the speaker's meaning and intention as closely as possible.

Here is the full transcript for context:
-------
FULL TRANSCRIPTION TEXT START: 

{full_transcription_text_in_original_language}

FULL TRANSCRIPTION TEXT END

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


def format_segment_level_translation_system_prompts(
        full_text_translated_transcript: TranslatedTranscription,
        original_language: LanguageNameString = "ENGLISH"
) -> dict[LanguageNameString, list[str]]:
    segment_level_prompts_by_language = {}
    for segment_number, segment in enumerate(full_text_translated_transcript.segments):
        for language_name, language_config in get_language_configs().items():
            # if original_language.lower() in language_name.lower():
            #     continue
            if language_name not in segment_level_prompts_by_language:
                segment_level_prompts_by_language[language_name] = []
            segment_level_prompts_by_language[language_name].append(SEGMENT_LEVEL_TRANSCRIPT_TRANSLATION_SYSTEM_PROMPT.format(
                original_language=full_text_translated_transcript.original_language,
                target_language_with_their_romanization_methods=language_config.model_dump_json(indent=2, exclude={"annotation_config"}),
                full_transcription_text_in_original_language=full_text_translated_transcript.original_text,
                segment_number=segment_number,
                total_segments=len(full_text_translated_transcript.segments),
                current_segment_in_original_language=segment.original_segment_text,
                start_timestamp=segment.start,
                end_timestamp=segment.end,
                duration=full_text_translated_transcript.segments[-1].end,
            ))

    logger.debug(
        "==================================\n\n"
        f"Formatted system prompts: \n\n"
        f" FULL TEXT/SEGMENT LEVEL SYSTEM PROMPT: \n"
        f"{segment_level_prompts_by_language}\n"
        "==================================\n\n"
    )

    return segment_level_prompts_by_language

