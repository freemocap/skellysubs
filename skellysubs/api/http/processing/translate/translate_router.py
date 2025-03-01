import logging

from fastapi import APIRouter, HTTPException
from openai.types.audio import TranscriptionVerbose

from skellysubs.core.audio_transcription.whisper_transcript_result_model import WhisperTranscriptionResult
from skellysubs.core.translation_pipeline.models.translated_text_models import TranslatedText
from skellysubs.core.translation_pipeline.models.translated_transcript_model import TranslatedTranscription
from skellysubs.core.translation_pipeline.translate_transcription_pipeline import translate_transcription_pipeline

logger = logging.getLogger(__name__)
translate_router = APIRouter()





@translate_router.post("/translate", response_model=TranslatedTranscription)
async def translate_transcript_endpoint(
        transcription: TranscriptionVerbose
) -> TranslatedTranscription:
    logger.info(f"Translation request received for transcription: {transcription.text}")

    try:


        translation =await translate_transcription_pipeline(og_transcription=WhisperTranscriptionResult.from_verbose_transcript(transcription))
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        logger.exception(f"Transcription initialization failed - {e}")
        raise HTTPException(status_code=500, detail="Internal server error")
    logger.info(f"Returning translation: {translation}")
    return translation
