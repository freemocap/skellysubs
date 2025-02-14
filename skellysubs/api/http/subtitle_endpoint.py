import logging
import subprocess
from pathlib import Path

from fastapi import APIRouter, BackgroundTasks, HTTPException, UploadFile, File
from starlette.responses import JSONResponse

from skellysubs.__main__ import run_video_subtitle_pipeline

logger = logging.getLogger(__name__)
subtitle_router = APIRouter()


async def background_video_task(video_path: Path):
    try:
        await run_video_subtitle_pipeline(video_name=str(video_path))
        logger.info("Video processed successfully!")
    except Exception as e:
        logger.exception(f"Error processing video: {e}")
        raise HTTPException(status_code=500, detail=f"Error processing video: {e}")


async def validate_video_file(file: UploadFile):
    # Check MIME type
    if not file.content_type.startswith('video/'):
        raise HTTPException(status_code=400, detail="File is not a valid video type.")

    # Check file extension
    valid_extensions = ['.mp4', '.avi', '.mov', '.mkv', '.webm']
    if not any(file.filename.endswith(ext) for ext in valid_extensions):
        raise HTTPException(status_code=400, detail="File extension is not supported.")


async def validate_video_with_ffmpeg(file_path: str):
    try:
        # Run ffprobe to check the file
        result = subprocess.run(
            ['ffprobe', '-v', 'error', '-show_entries', 'format=duration', '-of', 'default=noprint_wrappers=1:nokey=1',
             file_path],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE
        )
        if result.returncode != 0:
            raise HTTPException(status_code=400, detail="File is not a valid video.")
    except Exception as e:
        raise HTTPException(status_code=400, detail=f"Error processing video file: {str(e)}")




@subtitle_router.post("/subtitle_video", summary="Transcribe, translate, and add subtitles to video")
async def subtitle_video_endpoint(background_tasks: BackgroundTasks, file: UploadFile = File(...)):
    """
    Endpoint to process a video file by adding subtitles.
    Accepts a video file upload.
    """
    # Validate the uploaded file
    await validate_video_file(file)

    # Define the path to save the uploaded video file
    upload_dir = Path().home() / "skellysubs-data" / "uploads" / f"{Path(file.filename).stem}"
    upload_dir.mkdir(parents=True, exist_ok=True)  # Ensure the directory exists

    video_path = upload_dir  / file.filename

    # Save the uploaded video file
    with video_path.open("wb") as buffer:
        buffer.write(await file.read())

    # Validate the saved file with ffmpeg
    await validate_video_with_ffmpeg(str(video_path))


    # Add the task to the background
    background_tasks.add_task(background_video_task, video_path)

    return JSONResponse(content={"message": "Video processing started in background!"}, status_code=202)
