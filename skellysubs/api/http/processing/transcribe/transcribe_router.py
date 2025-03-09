import logging
import os
import uuid

from fastapi import APIRouter, File, UploadFile, HTTPException, Form
from openai.types.audio import TranscriptionVerbose
from pydantic import BaseModel

from skellysubs.ai_clients.openai_client import get_or_create_openai_client
from skellysubs.core.subtitles.formatters.base_subtitle_formatter import FormattedSubtitles
from skellysubs.core.subtitles.subtitle_generator import SubtitleGenerator

logger = logging.getLogger(__name__)
transcribe_router = APIRouter()


class ValidationResult(BaseModel):
    valid: bool
    reason: str | None = None


class TranscriptionResponse(BaseModel):
    transcript: TranscriptionVerbose
    formatted_subtitles: FormattedSubtitles


@transcribe_router.post("/transcribe", response_model=TranscriptionResponse)
async def transcribe_endpoint(
        audio_file: UploadFile = File(...),
        language: str = Form(None),
        prompt: str = Form(None)
) -> TranscriptionResponse | None:
    logger.info(f"Transcription request received for file: {audio_file.filename}")
    audio_temp_filename = f"temp_{uuid.uuid4()}_{audio_file.filename}"
    subtitle_generator = SubtitleGenerator()

    try:
        with open(audio_temp_filename, "wb") as incoming_f:
            incoming_f.write(audio_file.file.read())

        with open(audio_temp_filename, "rb") as f:
            transcription_result = await get_or_create_openai_client().make_whisper_transcription_request(
                audio_file=f,
                language=language,
                prompt=prompt
            )
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        logger.exception(f"Transcription initialization failed - {e}")
        raise HTTPException(status_code=500, detail=f"Internal server error: {e}")
    finally:
        os.remove(audio_temp_filename)
    logger.info(f"Returning transcription! {len(transcription_result.text.strip(' '))} words")
    return TranscriptionResponse(transcript=transcription_result,
                                 formatted_subtitles=subtitle_generator.generate_all_formats(transcription_result))
