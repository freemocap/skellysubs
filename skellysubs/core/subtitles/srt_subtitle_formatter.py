from skellysubs.core.subtitles.base_subtitle_formatter import SubtitleFormatter
from openai.types.audio import TranscriptionVerbose

from skellysubs.core.translation.models.translated_transcript import TranslatedTranscript


class SRTFormatter(SubtitleFormatter):
    @staticmethod
    def format_time(seconds: float) -> str:
        hours = int(seconds // 3600)
        minutes = int((seconds % 3600) // 60)
        seconds_remainder = seconds % 60
        milliseconds = int((seconds_remainder - int(seconds_remainder)) * 1000)
        return f"{hours:02}:{minutes:02}:{int(seconds_remainder):02},{milliseconds:03}"

    def format_transcript(self, transcript: TranscriptionVerbose) -> str:
        srt_output = []
        for index, segment in enumerate(transcript.segments, start=1):
            if segment.start == 0:
                segment.start += 0.01  # SRT's get screwy if the start time is 0
            start_time = self.format_time(segment.start)
            end_time = self.format_time(segment.end)
            text = segment.text.strip()

            srt_entry = f"{index}\n{start_time} --> {end_time}\n{text}\n"
            srt_output.append(srt_entry)

        return "\n".join(srt_output)

    def format_translated_transcript(self, translated_transcript: TranslatedTranscript) -> dict[str, str]:
        variants = {}
        lang_config = translated_transcript.translated_language

        # Base translated version
        base_srt = []
        for idx, segment in enumerate(translated_transcript.translated_segments, 1):
            start = segment.start + 0.01 if segment.start == 0 else segment.start
            base_srt.append(
                f"{idx}\n{self.format_time(start)} --> {self.format_time(segment.end)}\n"
                f"{segment.translated_text.translated_text.strip()}"
            )
        variants['translated'] = "\n\n".join(base_srt)

        # Romanized version if needed
        if lang_config.romanization_method:
            variants['translated_with_romanization'] = self._format_romanized(translated_transcript)

        return variants

    def _format_romanized(self, translated_transcript: TranslatedTranscript) -> str:
        romanized_srt = []
        for idx, segment in enumerate(translated_transcript.translated_segments, 1):
            start = segment.start + 0.01 if segment.start == 0 else segment.start
            romanized_text = segment.translated_text.romanized_text.strip()
            translated_text = segment.translated_text.translated_text.strip()

            romanized_srt.append(
                f"{idx}\n{self.format_time(start)} --> {self.format_time(segment.end)}\n"
                f"{translated_text}\n{romanized_text}"
            )
        return "\n\n".join(romanized_srt)