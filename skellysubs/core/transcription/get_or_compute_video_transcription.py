import json
from pathlib import Path

from moviepy import VideoFileClip

from skellysubs.core.transcription.whisper_audio_transcription import transcribe_audio
from skellysubs.core.transcription.whisper_transcript_result_model import WhisperTranscriptionResult


async def get_or_compute_video_transcription(video_path: str,
                                             local_whisper: bool = False,
                                             re_transcribe: bool = False) -> WhisperTranscriptionResult:
    extension = Path(video_path).suffix
    audio_path = video_path.replace(f"{extension}", ".mp3")
    transcript_path = video_path.replace(f"{extension}", "_transcription.json")
    if Path(transcript_path).exists() and not re_transcribe:
        with open(transcript_path, 'r') as f:
            transcription_json = json.load(f)
        transcription_result = WhisperTranscriptionResult(**transcription_json)
    else:
        scrape_and_save_audio_from_video(audio_path, video_path)
        transcription_result = await transcribe_audio(audio_path=audio_path, local_whisper=local_whisper)
        Path(transcript_path).write_text(json.dumps(transcription_result.model_dump(), indent=4))
    return transcription_result


def scrape_and_save_audio_from_video(audio_path: str, video_path: str) -> None:
    if not Path(video_path).exists() or not Path(video_path).is_file():
        raise FileNotFoundError(f"File not found: {video_path}")
    video = VideoFileClip(video_path)
    audio = video.audio
    Path(audio_path).parent.mkdir(parents=True, exist_ok=True)
    audio.write_audiofile(audio_path)


if __name__ == "__main__":
    import asyncio

    asyncio.run(get_or_compute_video_transcription(video_path="../sample_data/short_video_short/short_video_short.mp4"))
