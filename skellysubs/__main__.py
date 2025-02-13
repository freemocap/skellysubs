import json
import urllib
from pathlib import Path

import requests

from skellysubs.core.translation_pipeline.get_video_and_output_paths import get_video_and_output_paths
from skellysubs.core.translation_pipeline.models.translated_transcript_model import TranslatedTranscription
from skellysubs.core.translation_pipeline.translate_video import translate_video
from skellysubs.core.video_annotator.annotate_video_with_subtitles import annotate_video_with_subtitles
import logging
logger = logging.getLogger(__name__)
def is_url(video_name: str) -> bool:
    # Check whether the given string is a URL
    if not video_name.startswith("http"):
        return False
    try:
        result = urllib.parse.urlparse(video_name)
        return all([result.scheme, result.netloc])
    except ValueError:
        return False

async def download_video(url: str, download_path: str) -> str:
    # Download video from URL and save it to a local file
    response = requests.get(url, stream=True)
    response.raise_for_status()
    with open(download_path, 'wb') as f:
        for chunk in response.iter_content(chunk_size=8192):
            f.write(chunk)
    return download_path

async def run_video_subtitle_pipeline(video_name: str) -> None:
    logger.info(f"Running video subtitle pipeline for {video_name}")
    if is_url(video_name):
        # If it's a URL, download the video temporarily
        logger.debug("Video name is a URL - downloading video...")
        video_url = video_name
        video_name = f"video_url_ending_with_{video_name.split('/')[-1][:-20]}".replace('/', '_')
        video_path = f"../sample_data/{video_name}/{video_name}.mp4"
        Path(video_path).resolve().parent.mkdir(parents=True, exist_ok=True)
        video_name = await download_video(url=video_url,
                                          download_path=video_path)
    (subtitled_video_path,
     video_path,
     translation_path) = await get_video_and_output_paths(video_path=video_name)


    if Path(translation_path).exists():
        logger.debug(f"Translation file already exists - loading from {translation_path}")
        with open(translation_path, 'r', encoding='utf-8') as f:
            transcription_json = json.load(f)
        translation_result = TranslatedTranscription(**transcription_json)
    else:
        logger.debug(f"Translation file does not exist - running translation pipeline...")
        translation_result = await translate_video(video_path=video_path)
        logger.debug(f"Translation pipeline complete!")
        # Save the translation result
        Path(video_path.replace('.mp4', '_translation.json')).write_text(translation_result.model_dump_json(indent=4), encoding='utf-8')
        logger.debug(f"Translation result saved to {translation_path}")

        # Generate subtitle files
        subtitles_path = Path(video_path).parent / 'subtitle_files'
        subtitles_path.mkdir(parents=True, exist_ok=True)
        logger.debug(f"Generating subtitle files in {subtitles_path}")
        subtitle_basename = Path(video_path).stem.replace('.mp4', '')
        translation_result.generate_subtitle_files(file_basename=subtitle_basename,
                                                   subtitle_directory=str(subtitles_path))
        logger.debug(f"Translation pipeline complete!")
    logger.debug(f"Starting video annotation...")

    # Annotate the video with the translated words
    annotate_video_with_subtitles(video_path = video_path,
                                  translated_transcript = translation_result,
                                  subtitled_video_path = subtitled_video_path)

if __name__ == '__main__':
    import asyncio
    video_paths = [
        "../sample_data/2025-02-12-jsm-video/2025-02-12-jsm-video.mp4",
        "../sample_data/sample_video_short/sample_video_short.mp4",
        "../sample_data/sample_video_long/sample_video_long.mp4",
        "../sample_data/start-here-1/start-here-1.mp4",
        "../sample_data/2025-01-27-jsm-video/2025-01-27-jsm-video.mp4",
        "https://github.com/user-attachments/assets/0bc27df0-9614-4716-8638-f0b130ef791d",
        "../sample_data/this-is-freemocap/this-is-freemocap.mp4",  # https://www.youtube.com/watch?v=WW_WpMcbzns
    ]
    only_process_first_video = True
    fail_on_error = True
    for video_name in video_paths:
        try:
            if not Path(video_name).exists():
                raise ValueError(f"Video {video_name} does not exist!")
            asyncio.run(run_video_subtitle_pipeline(video_name=video_name))
            if only_process_first_video:
                break
        except Exception as e:
            logger.exception(f"Error processing video {video_name}: {e}")
            if fail_on_error:
                raise

    logger.info(f"Done!")
