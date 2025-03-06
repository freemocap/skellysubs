from openai.types.audio import TranscriptionVerbose

from skellysubs.core.translation_pipeline.models.translated_text_models import TranslatedTranscript

SrtFormatedString = str
def convert_transcript_to_srt(transcription_verbose: TranscriptionVerbose) -> SrtFormatedString:
    def format_time(seconds):
        hours = int(seconds // 3600)
        minutes = int((seconds % 3600) // 60)
        seconds = seconds % 60
        milliseconds = (seconds - int(seconds)) * 1000
        return f"{hours:02}:{minutes:02}:{int(seconds):02},{int(milliseconds):03}"

    srt_output = []
    for index, segment in enumerate(transcription_verbose.segments, start=1):
        if segment.start == 0:
            segment.start += 0.01 # Srt's get screwy if the start time is 0, so bump it up a bit
        start_time = format_time(segment.start)
        end_time = format_time(segment.end)
        text = segment.text.strip()

        srt_entry = f"{index}\n{start_time} --> {end_time}\n{text}\n"
        srt_output.append(srt_entry)

    return "\n".join(srt_output)


def format_time(seconds: float) -> str:
    hours = int(seconds // 3600)
    minutes = int((seconds % 3600) // 60)
    seconds_remainder = seconds % 60
    milliseconds = int((seconds_remainder - int(seconds_remainder)) * 1000)
    return f"{hours:02}:{minutes:02}:{int(seconds_remainder):02},{milliseconds:03}"


def convert_translated_transcript_to_srt(translated_transcript: TranslatedTranscript) -> dict[str, SrtFormatedString]:
    srt_dict = {}
    lang_config = translated_transcript.translated_language

    # Always create base translated version
    base_srt = []
    for idx, segment in enumerate(translated_transcript.translated_segments, 1):
        start = segment.start + 0.01 if segment.start == 0 else segment.start
        base_srt.append(
            f"{idx}\n{format_time(start)} --> {format_time(segment.end)}\n"
            f"{segment.translated_text.translated_text.strip()}"
        )
    srt_dict['translated'] = "\n\n".join(base_srt)

    # Create romanized version if needed
    if lang_config.romanization_method:
        romanized_srt = []
        for idx, segment in enumerate(translated_transcript.translated_segments, 1):
            start = segment.start + 0.01 if segment.start == 0 else segment.start
            romanized_text = segment.translated_text.romanized_text.strip()
            translated_text = segment.translated_text.translated_text.strip()

            romanized_srt.append(
                f"{idx}\n{format_time(start)} --> {format_time(segment.end)}\n"
                f"{translated_text}\n{romanized_text}"
            )
        srt_dict['translated_with_romanization'] = "\n\n".join(romanized_srt)

    return srt_dict
if __name__ == "__main__":
    import json
    from skellysubs import SAMPLE_DATA_TRANSCRIPT_PATH
    with open(SAMPLE_DATA_TRANSCRIPT_PATH) as f:
        sample_transcript = TranscriptionVerbose(**json.load(f))

    print(convert_transcript_to_srt(sample_transcript))