import logging
import os
import uuid
from typing import BinaryIO

from fastapi import APIRouter, File, UploadFile, HTTPException
from fastapi.responses import JSONResponse
from pydantic import BaseModel

from skellysubs.ai_clients.openai_client import get_or_create_openai_client
from skellysubs.api.websocket.websocket_server import WebsocketPayload
from skellysubs.app.skellysubs_app_state import get_skellysubs_app_state
from openai.types.audio import TranscriptionVerbose

logger = logging.getLogger(__name__)
transcribe_router = APIRouter()



class ValidationResult(BaseModel):
    valid: bool
    reason: str | None = None


def _validate_audio_file(audio_file: BinaryIO) -> ValidationResult:
    # TODO - Validate file type, size, etc. Ma
    # accept = ['flac', 'm4a', 'mp3', 'mp4', 'mpeg', 'mpga', 'oga', 'ogg', 'wav', 'webm']"
    # accept < 25MB
    return ValidationResult(valid=True)


@transcribe_router.post("/transcribe", response_model=TranscriptionVerbose)
async def transcribe_endpoint(
        audio_file: UploadFile = File(...),
) -> JSONResponse | HTTPException:
    try:

        audio_temp_filename = f"temp_{uuid.uuid4()}_{audio_file.filename}"
        with open(audio_temp_filename, "wb") as incoming_f:
            incoming_f.write(audio_file.file.read())


        with open(audio_temp_filename, "rb") as f:
            audio_validation = _validate_audio_file(f)
            if not audio_validation.valid:
                return HTTPException(status_code=400, detail=audio_validation.reason)

            result = await get_or_create_openai_client().make_whisper_transcription_request(
                audio_file=f,
            )

        return JSONResponse(status_code=200, content=result.model_dump())
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        logger.exception("Transcription initialization failed - {e}")
        raise HTTPException(status_code=500, detail=f"Internal server error")
