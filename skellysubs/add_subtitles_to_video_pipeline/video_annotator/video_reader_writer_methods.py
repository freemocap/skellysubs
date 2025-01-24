import os
from pathlib import Path

import PIL
import cv2
import numpy as np
from PIL.Image import Image

from skellysubs.translate_transcript_pipeline.transcribe_video import scrape_and_save_audio_from_video


def create_video_reader_and_writer(subtitled_video_path, video_path):
    # Load the video
    if not Path(video_path).exists() or not Path(video_path).is_file():
        raise FileNotFoundError(f"File not found: {video_path}")
    video_reader = cv2.VideoCapture(video_path)
    video_width = int(video_reader.get(cv2.CAP_PROP_FRAME_WIDTH))
    video_height = int(video_reader.get(cv2.CAP_PROP_FRAME_HEIGHT))
    video_resolution = (video_width, video_height)
    video_framerate = video_reader.get(cv2.CAP_PROP_FPS)
    Path(subtitled_video_path).parent.mkdir(parents=True, exist_ok=True)
    if not subtitled_video_path.endswith('.mp4'):
        raise ValueError(f"Output path must end with .mp4: {subtitled_video_path}")
    no_audio_video_path = subtitled_video_path.replace('.mp4', '_no_audio.mp4')
    video_writer = cv2.VideoWriter(no_audio_video_path, cv2.VideoWriter_fourcc(*'x264'), video_framerate,
                                   video_resolution)
    if not video_writer.isOpened():
        raise ValueError(f"Failed to open video writer: {subtitled_video_path}")
    return no_audio_video_path, video_height, video_reader, video_width, video_writer


def write_frame_to_video_file(pil_image: Image,
                              video_writer: cv2.VideoWriter) -> np.ndarray:
    # Convert the annotated image back to a cv2 image and write it to the video
    image = cv2.cvtColor(np.array(pil_image), cv2.COLOR_RGB2BGR)
    if not video_writer.isOpened():
        raise ValueError(f"Video writer is not open before writing frame!")
    video_writer.write(image)
    if not video_writer.isOpened():
        raise ValueError(f"Video writer is not open after writing frame!")
    return image

import logging
logger = logging.getLogger(__name__)

def finish_video_and_attach_audio_from_original(original_video_path: str,
                                                no_audio_video_path: str,
                                                subtitled_video_path: str) -> None:
    # Save the audio from the original video to a wav file (if it doesn't already exist)
    original_audio_path = original_video_path.replace('.mp4', '.wav')
    if not Path(original_audio_path).exists():
        scrape_and_save_audio_from_video(original_video_path, original_audio_path)

    # Compress and combine the audio from the original video with the annotated video
    command = (
        f"ffmpeg -y -i {no_audio_video_path} -i {original_audio_path} "
        f"-c:v libx264 -crf 23 -preset fast -c:a aac -strict experimental "
        f"{subtitled_video_path}"
    )
    logger.info(f"Combining and compressing video and audio with ffmpeg command - {command}")
    os.system(command)

    # Delete the no-audio temporary video file
    try:
        os.remove(no_audio_video_path)
        logger.info(f"Deleted temporary no-audio video file: {no_audio_video_path}")
    except OSError as e:
        logger.error(f"Error deleting temporary video file {no_audio_video_path}: {e}")
