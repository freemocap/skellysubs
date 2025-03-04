from openai.types.audio import TranscriptionVerbose

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


if __name__ == "__main__":
    import json
    from skellysubs import SAMPLE_DATA_TRANSCRIPT_PATH
    with open(SAMPLE_DATA_TRANSCRIPT_PATH) as f:
        sample_transcript = TranscriptionVerbose(**json.load(f))

    print(convert_transcript_to_srt(sample_transcript))