import logging
import os
import uuid
from typing import BinaryIO

from fastapi import APIRouter, File, UploadFile, HTTPException, Form
from openai.types.audio import TranscriptionVerbose
from pydantic import BaseModel

from skellysubs.ai_clients.openai_client import get_or_create_openai_client
from skellysubs.core.audio_transcription.whisper_transcript_result_model import WhisperTranscriptionResult

logger = logging.getLogger(__name__)
transcribe_router = APIRouter()



class ValidationResult(BaseModel):
    valid: bool
    reason: str | None = None




@transcribe_router.post("/transcribe", response_model=TranscriptionVerbose)
async def transcribe_endpoint(
        audio_file: UploadFile = File(...),
        language: str = Form(None),
        prompt: str = Form(None)
) -> TranscriptionVerbose|None:
    logger.info(f"Transcription request received for file: {audio_file.filename}")
    audio_temp_filename = f"temp_{uuid.uuid4()}_{audio_file.filename}"
    try:
        with open(audio_temp_filename, "wb") as incoming_f:
            incoming_f.write(audio_file.file.read())

        with open(audio_temp_filename, "rb") as f:
            transcription_result = await get_or_create_openai_client().make_whisper_transcription_request(
                audio_file=f,
            )
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        logger.exception(f"Transcription initialization failed - {e}")
        raise HTTPException(status_code=500, detail="Internal server error")
    finally:
        os.remove(audio_temp_filename)
    logger.info(f"Returning transcription: {transcription_result}")
    return transcription_result
