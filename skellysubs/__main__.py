import json
from pathlib import Path

from skellysubs.add_subtitles_to_video_pipeline.get_video_and_output_paths import get_video_and_output_paths
from skellysubs.add_subtitles_to_video_pipeline.video_annotator.annotate_video_with_subtitles import \
    annotate_video_with_subtitles
from skellysubs.translate_transcript_pipeline.translate_video import translate_video
from skellysubs.translate_transcript_pipeline.models.translated_transcript_model import TranslatedTranscription


async def run_video_subtitle_pipeline(video_name: str) -> None:

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

    # Annotate the video with the translated words
    annotate_video_with_subtitles(video_path = video_path,
                                  translated_transcript = translation_result,
                                  subtitled_video_path = subtitled_video_path)

if __name__ == '__main__':
    import asyncio
    outer_video_name = str(Path("../sample_data/sample_video_short/sample_video_short.mp4").resolve())
    # outer_video_name = str(Path("../sample_data/sample_video_long/sample_video_long").resolve())
    # outer_video_name = str(Path("../sample_data/start-here-1/start-here-1").resolve())
    asyncio.run(run_video_subtitle_pipeline(video_name=outer_video_name))

