import logging

from fastapi import APIRouter, HTTPException
from fastapi import Body, Query
from pydantic import BaseModel

from skellysubs.core.audio_transcription.whisper_transcript_result_model import WhisperTranscriptionResult
from skellysubs.core.translation_pipeline.language_configs.language_configs import LanguageConfig
from skellysubs.core.translation_pipeline.models.translated_text_models import TranslatedText
from skellysubs.core.translation_pipeline.models.translated_transcript_model import TranslatedTranscription
from skellysubs.core.translation_pipeline.models.translation_typehints import LanguageNameString
from skellysubs.core.translation_pipeline.translate_transcription_pipeline import full_text_translation

logger = logging.getLogger(__name__)

translate_router = APIRouter()

class TranslationRequest(BaseModel):
    text: str
    target_languages: dict[LanguageNameString, LanguageConfig]

class TranslationResponse(BaseModel):
    prompts: dict[LanguageNameString, str]
    translations: dict[LanguageNameString, TranslatedText]

@translate_router.post("/translate/text", response_model=TranslationResponse)
async def translate_text_endpoint(
        text: str = Body(...),
        target_languages: dict[LanguageNameString, LanguageConfig] = Body(...),
        original_language: str = Query("english")
) -> TranslationResponse:
    logger.info(f"Translation request received for transcription: {text} with configs: {[key for key in target_languages.keys()]}")

    try:

        prompts, translations = await full_text_translation(text=text, target_languages=target_languages,original_language=original_language)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        logger.exception(f"Transcription initialization failed - {e}")
        raise HTTPException(status_code=500, detail="Internal server error")
    logger.info(f"Returning translations: {translations}")
    return TranslationResponse(prompts=prompts,
                               translations=translations)
