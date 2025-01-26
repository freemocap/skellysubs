import logging
from pathlib import Path

from fastapi import APIRouter, BackgroundTasks, HTTPException, UploadFile, File
from starlette.responses import JSONResponse

from skellysubs.__main__ import run_video_subtitle_pipeline

logger = logging.getLogger(__name__)
subtitle_router = APIRouter()

SAMPLE_VIDEO_PATH = "../sample_data/sample_video_short/sample_video_short.mp4"
# SAMPLE_VIDEO_PATH = "../../../sample_data/sample_video_long/sample_video_long.mp4"
if not Path(SAMPLE_VIDEO_PATH).resolve().exists():
    raise FileNotFoundError(f"Sample video not found at path: {Path(SAMPLE_VIDEO_PATH)}")


async def background_video_task(video_path: Path):
    try:
        await run_video_subtitle_pipeline(video_name=str(video_path))
        logger.info("Video processed successfully!")
    except Exception as e:
        logger.error(f"Error processing video: {e}")


@subtitle_router.post("/subtitle_video", summary="Transcribe, translate, and add subtitles to video")
async def subtitle_video_endpoint(background_tasks: BackgroundTasks, file: UploadFile = File(...)):
    """
    Endpoint to process a video file by adding subtitles.
    Accepts a video file upload.
    """
    # Define the path to save the uploaded video file
    upload_dir = Path().home() / "skellysubs-data" / "uploads"
    upload_dir.mkdir(parents=True, exist_ok=True)  # Ensure the directory exists

    video_path = upload_dir / f"{file.filename}"


    # Save the uploaded video file
    with video_path.open("wb") as buffer:
        buffer.write(await file.read())

    # Validate the saved file path
    if not video_path.exists():
        raise HTTPException(status_code=400, detail="Failed to save uploaded video file")

    # Add the task to the background
    background_tasks.add_task(background_video_task, video_path)

    return JSONResponse(content={"message": "Video processing started in background!"}, status_code=202)
