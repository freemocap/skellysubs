import logging
from pathlib import Path

import cv2

from skellysubs.audio_transcription.whisper_transcript_result_full_model import WhisperTranscriptionResult

logging.basicConfig(level=logging.DEBUG)
logger = logging.getLogger(__name__)


def validate_audio_path(audio_path: str) -> None:
    if not Path(audio_path).exists():
        raise FileNotFoundError(f"File not found: {audio_path}")
    if not Path(audio_path).is_file():
        raise ValueError(f"Path is not a file: {audio_path}")
    if not Path(audio_path).suffix in [".mp3", ".ogg", ".wav"]:
        raise ValueError(f"Unsupported file format: {audio_path}")


def transcribe_audio(audio_path: str, model_name: str = "large") -> WhisperTranscriptionResult:
    import whisper

    validate_audio_path(audio_path)
    model = whisper.load_model(model_name)
    result = model.transcribe(audio_path,
                              word_timestamps=True,)
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


