import logging

from fastapi import APIRouter, HTTPException
from openai.types.audio import TranscriptionVerbose

from skellysubs.core.audio_transcription.whisper_transcript_result_model import WhisperTranscriptionResult
from skellysubs.core.translation_pipeline.language_configs.language_configs import LanguageConfig
from skellysubs.core.translation_pipeline.models.translated_transcript_model import TranslatedTranscription
from skellysubs.core.translation_pipeline.models.translation_typehints import LanguageNameString
from skellysubs.core.translation_pipeline.translate_transcription_pipeline import  full_text_translation

logger = logging.getLogger(__name__)
translate_router = APIRouter()


@translate_router.post("/translate/text", response_model=TranslatedTranscription)
async def translate_text_endpoint(
        text: str,
        target_languages: dict[LanguageNameString, LanguageConfig],
        original_language: str = "english"
) -> TranslatedTranscription:
    logger.info(f"Translation request received for transcription: {text} with configs: {[key for key in target_languages.keys()]}")

    try:
        initialized_transcription = TranslatedTranscription.initialize(og_transcription=WhisperTranscriptionResult.from_verbose_transcript(transcription),
                                                                       original_langauge=original_language)
        translation = await full_text_translation(initialized_transcription=initialized_transcription)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        logger.exception(f"Transcription initialization failed - {e}")
        raise HTTPException(status_code=500, detail="Internal server error")
    logger.info(f"Returning translation: {translation}")
    return translation
