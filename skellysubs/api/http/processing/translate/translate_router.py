import logging

from fastapi import APIRouter, HTTPException
from openai.types.audio import TranscriptionVerbose

from skellysubs.core.audio_transcription.whisper_transcript_result_model import WhisperTranscriptionResult
from skellysubs.core.translation_pipeline.models.translated_transcript_model import TranslatedTranscription
from skellysubs.core.translation_pipeline.translate_transcription_pipeline import translate_transcription_pipeline, \
    full_text_translation

logger = logging.getLogger(__name__)
translate_router = APIRouter()





@translate_router.post("/translate", response_model=TranslatedTranscription)
async def translate_transcript_endpoint(
        transcription: TranscriptionVerbose
) -> TranslatedTranscription:
    logger.info(f"Translation request received for transcription: {transcription.text}")

    try:


        initialized_transcription = TranslatedTranscription.initialize(og_transcription=WhisperTranscriptionResult.from_verbose_transcript(transcription),
                                                                       original_langauge="english")
        translation = await full_text_translation(initialized_transcription=initialized_transcription)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        logger.exception(f"Transcription initialization failed - {e}")
        raise HTTPException(status_code=500, detail="Internal server error")
    logger.info(f"Returning translation: {translation}")
    return translation
