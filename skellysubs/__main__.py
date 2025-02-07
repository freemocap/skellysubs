import json
import urllib
from pathlib import Path

import requests

from skellysubs.translation_pipeline.get_video_and_output_paths import get_video_and_output_paths
from skellysubs.translation_pipeline.translate_video import translate_video
from skellysubs.translation_pipeline.models.translated_transcript_model import TranslatedTranscription
from skellysubs.video_annotator.annotate_video_with_subtitles import annotate_video_with_subtitles

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
    if is_url(video_name):
        # If it's a URL, download the video temporarily
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
        with open(translation_path, 'r', encoding='utf-8') as f:
            transcription_json = json.load(f)
        translation_result = TranslatedTranscription(**transcription_json)
    else:
        translation_result = await translate_video(video_path=video_path)
        # Save the translation result
        Path(video_path.replace('.mp4', '_translation.json')).write_text(translation_result.model_dump_json(indent=4), encoding='utf-8')
        subtitles_path = Path(video_path).parent / 'subtitle_files'
        subtitles_path.mkdir(parents=True, exist_ok=True)
        subtitle_basename = Path(video_path).stem.replace('.mp4', '')
        translation_result.generate_subtitle_files(file_basename=subtitle_basename,
                                              subtitle_directory=str(subtitles_path))


    # Annotate the video with the translated words
    annotate_video_with_subtitles(video_path = video_path,
                                  translated_transcript = translation_result,
                                  subtitled_video_path = subtitled_video_path)

if __name__ == '__main__':
    import asyncio
    outer_video_name = str(Path("../sample_data/sample_video_short/sample_video_short.mp4").resolve())
    # outer_video_name = str(Path("../sample_data/sample_video_long/sample_video_long").resolve())
    # outer_video_name = str(Path("../sample_data/start-here-1/start-here-1").resolve())
    # outer_video_name = str(Path("../sample_data/2025-01-27-jsm-video/2025-01-27-jsm-video.mp4").resolve())
    # outer_video_name = str(Path(r"D:\videos\obs-recordings\2025-02-01\2025-02-01T17-50gmt-0500\2025-02-01T17-50gmt-0500-3840x2160-30fps-NV12_vertical.mp4").resolve())
    # outer_video_name = str(Path(r"C:\Users\jonma\Sync\videos\social-media-posts\2025-02-04-skellysubs-3min\2025-02-04-skellysubs-3min-raw.mp4").resolve())
    # outer_video_name = "https://github.com/user-attachments/assets/0bc27df0-9614-4716-8638-f0b130ef791d"


    asyncio.run(run_video_subtitle_pipeline(video_name=outer_video_name))

