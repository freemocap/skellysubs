from datetime import timedelta
from pathlib import Path

import  logging
logger = logging.getLogger(__name__)

def generate_vtt_files(transcription, file_basename: str, save_directory: str):
    """Generate VTT file content from a TranslatedTranscription object."""
    logger.info(f"Generating VTT files for {file_basename} in {save_directory}")
    save_directory = Path(save_directory)
    save_directory.mkdir(parents=True, exist_ok=True)

    def format_vtt_time(seconds: float) -> str:
        """Convert seconds to VTT timestamp format."""
        if seconds <= 0:
            seconds = 0.1
        td = timedelta(seconds=seconds)
        return str(td)[:-3] + '.000'

    for language_name in transcription.full_text_translations.full_text_translations.keys():
        vtt_content = ["WEBVTT\n"]
        vtt_content_with_romanization = ["WEBVTT\n"]
        for index, segment in enumerate(transcription.segments, start=1):
            start_time = format_vtt_time(segment.start)
            end_time = format_vtt_time(segment.end)
            segment_word_list, romanized_word_list = segment.get_word_list_by_language(language_name)

            vtt_block = f"{start_time} --> {end_time} line:50%\n{' '.join(segment_word_list)}\n"
            vtt_content.append(vtt_block)

            if romanized_word_list:
                vtt_block_with_romanization = vtt_block + ' '.join(romanized_word_list) + '\n'
                vtt_content_with_romanization.append(vtt_block_with_romanization)

        vtt_filename = save_directory / f"{file_basename}_{language_name}.vtt"
        with open(vtt_filename, 'w', encoding='utf-8') as f:
            f.write('\n'.join(vtt_content))

        if len(vtt_content_with_romanization) > 1:
            vtt_filename_with_romanization = save_directory / f"{file_basename}_{language_name}_with_romanization.vtt"
            with open(vtt_filename_with_romanization, 'w', encoding='utf-8') as f:
                f.write('\n'.join(vtt_content_with_romanization))


def generate_ttml_files(transcription, file_basename: str, save_directory: str):
    """Generate TTML file content from a TranslatedTranscription object."""
    logger.info(f"Generating TTML files for {file_basename} in {save_directory}")
    save_directory = Path(save_directory)
    save_directory.mkdir(parents=True, exist_ok=True)

    def format_ttml_time(seconds: float) -> str:
        """Convert seconds to TTML timestamp format."""
        if seconds <= 0:
            seconds = 0.1
        td = timedelta(seconds=seconds)
        hours, remainder = divmod(td.seconds, 3600)
        minutes, seconds = divmod(remainder, 60)
        return f"{hours:02}:{minutes:02}:{seconds:02}.{td.microseconds // 1000:03}"

    for language_name in transcription.full_text_translations.full_text_translations.keys():
        ttml_content = [
            '<?xml version="1.0" encoding="UTF-8"?>',
            '<tt xmlns="http://www.w3.org/ns/ttml">',
            '<head>',
            '<layout>',
            '<region xml:id="bottomHalf" tts:origin="0% 50%" tts:extent="100% 50%"/>',
            '</layout>',
            '</head>',
            '<body><div>'
        ]
        ttml_content_with_romanization = ttml_content.copy()

        for index, segment in enumerate(transcription.segments, start=1):
            start_time = format_ttml_time(segment.start)
            end_time = format_ttml_time(segment.end)
            segment_word_list, romanized_word_list = segment.get_word_list_by_language(language_name)

            ttml_block = f'<p begin="{start_time}" end="{end_time}">{" ".join(segment_word_list)}</p>'
            ttml_content.append(ttml_block)

            if romanized_word_list:
                ttml_block_with_romanization = (
                    f'<p begin="{start_time}" end="{end_time}">'
                    f'{" ".join(segment_word_list)}<br/>{" ".join(romanized_word_list)}</p>'
                )
                ttml_content_with_romanization.append(ttml_block_with_romanization)

        ttml_content.append('</div></body></tt>')
        ttml_filename = save_directory / f"{file_basename}_{language_name}.ttml"
        with open(ttml_filename, 'w', encoding='utf-8') as f:
            f.write('\n'.join(ttml_content))

        if len(ttml_content_with_romanization) > 3:  # Check if there are any entries beyond the headers
            ttml_content_with_romanization.append('</div></body></tt>')
            ttml_filename_with_romanization = save_directory / f"{file_basename}_{language_name}_with_romanization.ttml"
            with open(ttml_filename_with_romanization, 'w', encoding='utf-8') as f:
                f.write('\n'.join(ttml_content_with_romanization))
