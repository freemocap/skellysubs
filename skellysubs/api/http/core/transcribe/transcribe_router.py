import logging
import os
import uuid

from fastapi import APIRouter, BackgroundTasks, File, UploadFile, HTTPException
from fastapi.responses import HTMLResponse
from pydantic import BaseModel

from skellysubs.ai_clients.openai_client import get_or_create_openai_client
from skellysubs.api.websocket.websocket_server import WebsocketPayload
from skellysubs.app.skellysubs_app_state import get_skellysubs_app_state

logger = logging.getLogger(__name__)
transcribe_router = APIRouter()


async def background_transcription_task(
        audio_file: bytes,
        audio_file_name: str,
        session_id: uuid.UUID
):
    try:
        result = await get_or_create_openai_client().make_whisper_transcription_request(
            audio_file=audio_file,
        )
        get_skellysubs_app_state().websocket_queue.put_nowait(WebsocketPayload(
            session_id=session_id,
            payload=result
        ))
        logger.info("Transcription processed successfully!")
    except Exception as e:
        logger.exception(f"Transcription failed - {e}")
        raise
    finally:
        # Clean up temp file
        try:
            os.remove(audio_file_name)
        except Exception as e:
            logger.exception(f"Failed to remove temp file {audio_file_name}")


class ValidationResult(BaseModel):
    valid: bool
    reason: str | None = None


def _validate_audio_file(audio_file: UploadFile):
    # Validate file type, size, etc. Ma
    # accept = ['flac', 'm4a', 'mp3', 'mp4', 'mpeg', 'mpga', 'oga', 'ogg', 'wav', 'webm']"
    # accept < 25MB
    return ValidationResult(valid=True)


def _validate_session_id(session_id: uuid.UUID):
    # Validate session_id, ensure its a valid UUID
    return ValidationResult(valid=True)


@transcribe_router.post("/transcribe")
async def transcribe_endpoint(
        background_tasks: BackgroundTasks,
        audio_file: UploadFile = File(...),
        connection_id: uuid.UUID = uuid.uuid4()
):
    try:

        audio_validation = _validate_audio_file(audio_file)
        id_validation = _validate_session_id(connection_id)

        error_message: str = ""
        if not audio_validation.valid:
            error_message += f"Invalid audio file: {audio_validation.reason} \n"
        if not id_validation.valid:
            error_message += f"Invalid session_id: {id_validation.reason}"
        if error_message:
            return HTTPException(status_code=400, detail=error_message)

        temp_file_name = f"temp_{uuid.uuid4()}_{audio_file.filename}"
        with open(temp_file_name, "wb") as temp_file:
            temp_file.write(audio_file.file.read())
        temp_audio_file = open(temp_file_name, "rb")
        # Pass bytes to background task
        background_tasks.add_task(
            background_transcription_task,
            audio_file=temp_audio_file,
            audio_file_name=temp_file_name,
            session_id=connection_id
        )

        return HTMLResponse(status_code=202, content="Transcription request accepted", background=background_tasks)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        logger.exception("Transcription initialization failed")
        raise HTTPException(status_code=500, detail="Internal server error")
