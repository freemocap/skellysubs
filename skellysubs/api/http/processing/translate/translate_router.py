import logging

from fastapi import APIRouter, HTTPException
from fastapi import Body, Query
from openai.types.audio import TranscriptionVerbose
from pydantic import BaseModel

from skellysubs.core.subtitles.srt_format_subtitle_generator import SrtFormatedString, \
    convert_translated_transcript_to_srt
from skellysubs.core.translation_pipeline.language_configs.language_configs import LanguageConfig
from skellysubs.core.translation_pipeline.models.translated_text_models import TranslatedText, TranslatedTranscript
from skellysubs.core.translation_pipeline.models.translation_typehints import LanguageNameString
from skellysubs.core.translation_pipeline.translation_subtasks.translate_full_text import text_translation
from skellysubs.core.translation_pipeline.translation_subtasks.translate_transcript_segments import \
    transcript_translation

logger = logging.getLogger(__name__)

translate_router = APIRouter()


class TextTranslationRequest(BaseModel):
    text: str
    target_languages: dict[LanguageNameString, LanguageConfig]


class TextTranslationResponse(BaseModel):
    prompts: dict[LanguageNameString, list[str]]
    translations: dict[LanguageNameString, TranslatedText]


class TranscriptTranslationResponse(BaseModel):
    translated_transcripts: dict[LanguageNameString, TranslatedTranscript]
    segment_prompts_by_language: dict[LanguageNameString, list[str]]
    translated_srt_subtitles: dict[LanguageNameString, dict[str, SrtFormatedString]]


@translate_router.post("/translate/text", response_model=TextTranslationResponse)
async def translate_text_endpoint(
        text: str = Body(...),
        target_languages: dict[LanguageNameString, LanguageConfig] = Body(...),
        original_language: str = Query("english")
) -> TextTranslationResponse:
    logger.api(
        f"`/translate/text` - Translation request received for transcription ({len(text.split(' '))} words) for target languages: {[key for key in target_languages.keys()]}")

    try:

        prompts, translations = await text_translation(text=text,
                                                       target_languages=target_languages,
                                                       original_language=original_language)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        logger.exception(f"Transcription initialization failed - {e}")
        raise HTTPException(status_code=500, detail="Internal server error")
    logger.info(f"Returning translations: {translations}")
    return TextTranslationResponse(prompts=prompts,
                                   translations=translations)


@translate_router.post("/translate/transcript", response_model=TranscriptTranslationResponse)
async def translate_transcript_endpoint(
        transcript: TranscriptionVerbose = Body(...),
        target_languages: dict[LanguageNameString, LanguageConfig] = Body(...)) -> TranscriptTranslationResponse:
    logger.info(
        f"Translation request received for transcription with {len(transcript.segments)} segments and target languages: {[key for key in target_languages.keys()]}")
    translated_transcripts = {}
    translated_srt_subtitles = {}
    try:
        full_text_prompts, full_text_translations = await text_translation(text=transcript.text,
                                                                           target_languages=target_languages,
                                                                           original_language=transcript.language)

        segment_prompts_by_language, translated_segments_by_language = await transcript_translation(
            original_transcript=transcript,
            full_text_translations=full_text_translations,
            target_languages=target_languages,
        )
        for language, language_config in target_languages.items():
            translated_transcripts[language] = TranslatedTranscript(original_language=transcript.language,
                                                                    original_full_text=transcript.text,
                                                                    translated_language=language_config,
                                                                    translated_full_text=full_text_translations[
                                                                        language],
                                                                    translated_segments=translated_segments_by_language[
                                                                        language]
                                                                    )

        # Generate SRTs for each translation
        for language, translated_transcript in translated_transcripts.items():
            srt_variants = convert_translated_transcript_to_srt(translated_transcript)
            translated_srt_subtitles[language] = srt_variants
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        logger.exception(f"Transcription initialization failed - {e}")
        raise HTTPException(status_code=500, detail="Internal server error")
    logger.info(f"Transcription translation complete!")
    return TranscriptTranslationResponse(translated_transcripts=translated_transcripts,
                                         segment_prompts_by_language=segment_prompts_by_language,
                                         translated_srt_subtitles=translated_srt_subtitles
                                         )
