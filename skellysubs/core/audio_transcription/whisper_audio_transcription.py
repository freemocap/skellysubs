import logging
from pathlib import Path

import cv2

from skellysubs.ai_clients.openai_client import get_or_create_openai_client
from skellysubs.core.audio_transcription.whisper_transcript_result_model import WhisperTranscriptionResult

logging.basicConfig(level=logging.DEBUG)
logger = logging.getLogger(__name__)

TRANSCRIPTION_BASE_PROMPT ="Words you may hear, with their correct spelling: `Jon Matthis`, `Jonathan Samir Matthis`, `FreeMoCap`"

def validate_audio_path(audio_path: str) -> None:
    if not Path(audio_path).exists():
        raise FileNotFoundError(f"File not found: {audio_path}")
    if not Path(audio_path).is_file():
        raise ValueError(f"Path is not a file: {audio_path}")
    if not Path(audio_path).suffix in [".mp3", ".ogg", ".wav"]:
        raise ValueError(f"Unsupported file format: {audio_path}")


async def transcribe_audio(audio_path: str,
                           local_whisper: bool = False,
                           model_name: str = "large") -> WhisperTranscriptionResult:
    if local_whisper:
        return transcribe_audio_with_local_whisper(audio_path, model_name)
    else:
        return await transcribe_audio_openai(audio_path)

async def transcribe_audio_openai(audio_path: str) -> WhisperTranscriptionResult:
    validate_audio_path(audio_path)
    audio_file = open(audio_path, "rb")
    result = await get_or_create_openai_client().make_whisper_transcription_request(audio_file=audio_file, prompt=TRANSCRIPTION_BASE_PROMPT)
    return WhisperTranscriptionResult.from_verbose_transcript(result)

def transcribe_audio_with_local_whisper(audio_path: str, model_name: str = "large") -> WhisperTranscriptionResult:
    import whisper
    import torch
    logger.info(
        f"Transcribing audio: {audio_path} with whisper model: {model_name} - import torch; torch.cuda_is_available(): {torch.cuda.is_available()}")

    validate_audio_path(audio_path)
    model = whisper.load_model(model_name)
    result = model.transcribe(audio = audio_path,
                              word_timestamps=True,
                              temperature=0.0,
                              no_speech_threshold=0.5,
                              hallucination_silence_threshold=0.5,
                              initial_prompt=TRANSCRIPTION_BASE_PROMPT,
                              )
    return WhisperTranscriptionResult(**result)


def transcribe_audio_detailed(audio_path: str,
                              model_name: str = "turbo",
                              ):
    import whisper

    model = whisper.load_model(model_name)

    # validate/load audio and pad/trim it to fit 30 seconds
    validate_audio_path(audio_path)
    audio = whisper.load_audio(audio_path)
    audio = whisper.pad_or_trim(audio)

    # make log-Mel spectrogram and move to the same device as the model
    mel = whisper.log_mel_spectrogram(audio, n_mels=model.dims.n_mels).to(model.device)
    save_spectrogram_image(audio_path, mel)
    # detect the spoken language
    _, probs = model.detect_language(mel)
    print(f"Detected language: {max(probs, key=probs.get)}")

    # decode (transcribe) the audio
    transcription_result = whisper.decode(model=model,
                                          mel=mel,
                                          options=whisper.DecodingOptions())

    # print the recognized text
    print(transcription_result.text)
    return transcription_result


def save_spectrogram_image(audio_path, mel):
    mel_as_numpy = mel.cpu().numpy()
    mel_image = cv2.resize(mel_as_numpy, (4000, 1000))
    mel_image_scaled = cv2.normalize(mel_image, None, 0, 255, cv2.NORM_MINMAX)
    mel_image_heatmapped = cv2.applyColorMap(mel_image_scaled.astype('uint8'), cv2.COLORMAP_PLASMA)
    cv2.imwrite(str(Path(audio_path).with_suffix(".log_mel_spectrogram.png")), mel_image_heatmapped)
