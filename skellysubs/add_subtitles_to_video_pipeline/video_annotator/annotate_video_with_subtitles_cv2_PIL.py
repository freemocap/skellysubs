import logging
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

import cv2
import jieba
from PIL import Image, ImageFont, ImageDraw
from arabic_reshaper import arabic_reshaper
from bidi.algorithm import get_display
from tqdm import tqdm

from skellysubs.add_subtitles_to_video_pipeline.video_annotator.video_reader_writer_methods import \
    create_video_reader_and_writer, write_frame_to_video_file, finish_video_and_attach_audio_from_original
from skellysubs.translate_transcript_pipeline.language_models import LanguageNames
from skellysubs.translate_transcript_pipeline.translated_transcript_model import \
    TranslatedTranscription, TranslatedTranscriptSegmentWithWords, TranslatedWhisperWordTimestamp

logger = logging.getLogger(__name__)


@dataclass
class LanguageAnnotationConfig:
    language_name: LanguageNames
    font_path: str
    font_size: int
    buffer_size: int
    color: tuple[int, int, int]
    language_start_y: Callable[[int], int]
    language_font: ImageFont = None

    def __post_init__(self):
        if not Path(self.font_path).exists():
            raise FileNotFoundError(f"Font not found: {self.font_path}")
        self.language_font = ImageFont.truetype(self.font_path, self.font_size)


FONT_BASE_PATH = Path(__file__).parent.parent.parent.parent / "fonts"
LANGUAGE_ANNOTATION_CONFIGS = {
    LanguageNames.ENGLISH: LanguageAnnotationConfig(language_name=LanguageNames.ENGLISH,
                                                    font_path=str(FONT_BASE_PATH / "ARIAL.TTF"),
                                                    color=(27, 158, 119),
                                                    font_size=48,
                                                    language_start_y=lambda video_height: 0,
                                                    buffer_size=100),
    LanguageNames.SPANISH: LanguageAnnotationConfig(language_name=LanguageNames.SPANISH,
                                                    font_path=str(FONT_BASE_PATH / "ARIAL.TTF"),
                                                    font_size=48,
                                                    color=(217, 95, 2),
                                                    language_start_y=lambda video_height: int(video_height // 6),
                                                    buffer_size=100),
    LanguageNames.CHINESE_MANDARIN_SIMPLIFIED: LanguageAnnotationConfig(
        language_name=LanguageNames.CHINESE_MANDARIN_SIMPLIFIED,
        font_path=str(FONT_BASE_PATH / "NotoSerifCJKsc-VF-Simplified-Chinese.ttf"),
        font_size=48,
        color=(117, 112, 179),
        language_start_y=lambda video_height: int(video_height // 3),
        buffer_size=100),
    LanguageNames.ARABIC_LEVANTINE: LanguageAnnotationConfig(language_name=LanguageNames.ARABIC_LEVANTINE,
                                                             font_path=str(
                                                                 FONT_BASE_PATH / "NotoKufiArabic-Regular.otf"),
                                                             color=(231, 41, 138),
                                                             font_size=48,
                                                             language_start_y=lambda video_height: int(
                                                                 video_height // 1.35),
                                                             buffer_size=100),
}


def create_multiline_text_chinese(text: str, font: ImageFont, screen_width: int, buffer: int) -> str:
    """
    Break a long string of Chinese text into multiple lines of text that fit within the screen width.
    Uses jieba for segmentation.
    """
    words = list(jieba.cut(text))
    lines = []
    current_line = ""
    for word in words:
        if font.getlength(current_line + word) + 2 * buffer < screen_width:
            current_line += word
        else:
            lines.append(current_line)
            current_line = word
    lines.append(current_line)
    return '\n'.join(lines)


def create_multiline_text(text: str, font: ImageFont, screen_width: int, buffer: int) -> str:
    """
    Break a long string into multiple lines of text that fit within the screen width by inserting `\n` characters
    at appropriate locations. to ensure the text will fit within the screen width with `buffer` pixels of padding on each side.
    will use `font.getlength('word1 + ' + 'word2' ...) method to determine when to break lines.

    :param text: The text to break into multiple lines
    :param font: The font to use for the text
    :param screen_width: The width of the screen
    :param buffer: The number of pixels of padding to leave on each side of the text
    """
    words = text.split()
    lines = []
    current_line = ""
    for word in words:
        if font.getlength(current_line + ' ' + word) + 2 * buffer < screen_width:
            current_line += ' ' + word
        else:
            lines.append(current_line)
            current_line = word
    lines.append(current_line)
    return '\n'.join(lines)


def annotate_video_with_subtitles(video_path: str,
                                  translated_transcript: TranslatedTranscription,
                                  subtitled_video_path: str,
                                  show_while_annotating: bool = True
                                  ) -> None:
    (no_audio_video_path,
     video_height,
     video_reader,
     video_width,
     video_writer) = create_video_reader_and_writer(
        subtitled_video_path, video_path)

    video_number_of_frames = int(video_reader.get(cv2.CAP_PROP_FRAME_COUNT))
    try:
        # Go through each frame of the video and annotate it with the translated words based on their timestamps
        for frame_number in tqdm(range(video_number_of_frames), desc="Annotating video with subtitles",
                                 total=video_number_of_frames):
            if not video_reader.isOpened():
                raise ValueError(f"Video reader is not open: {video_path}")

            read_success, image = video_reader.read()
            image = cv2.cvtColor(image, cv2.COLOR_BGR2RGB)
            if not read_success or not video_reader.isOpened():
                break
            frame_number += 1
            frame_timestamp = video_reader.get(
                cv2.CAP_PROP_POS_MSEC) / 1000  # seconds - internally based on frame# * presumed frame duration based on specified framerate
            pil_image = Image.fromarray(image)
            image_annotator = ImageDraw.Draw(pil_image)

            current_segment, current_word = translated_transcript.get_segment_and_word_at_timestamp(frame_timestamp)
            highlighted_segment_texts_by_langauge = highlight_current_word(current_segment=current_segment,
                                                                           current_word=current_word)
            for language_name, config in LANGUAGE_ANNOTATION_CONFIGS.items():

                multiline_y_start = config.language_start_y(video_height)

                segment_text, romanized_segment_text = current_segment.get_text_by_language(language_name)

                current_word_text, current_word_romanized_text = current_word.get_word_by_language(language_name)

                # Arabic text reshaping and display
                if language_name.lower() == LanguageNames.ARABIC_LEVANTINE.value.lower():
                    reshaped_text = arabic_reshaper.reshape(segment_text)
                    segment_text_display = get_display(reshaped_text)
                else:
                    segment_text_display = segment_text

                highlighted_segment_text = highlighted_segment_texts_by_langauge[language_name]

                if language_name.lower() == LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
                    multiline_text = create_multiline_text_chinese(highlighted_segment_text['text'],
                                                                   config.language_font,
                                                                   video_width,
                                                                   config.buffer_size)
                else:
                    multiline_text = create_multiline_text(highlighted_segment_text['text'], config.language_font,
                                                           video_width,
                                                           config.buffer_size)
                romanized_multiline_text = None
                if highlighted_segment_text.get('romanized'):
                    romanized_segment_text = highlighted_segment_text['romanized']
                    romanized_multiline_text = create_multiline_text(romanized_segment_text, config.language_font,
                                                                     video_width, config.buffer_size)

                # Reverse lines for Arabic text to render correctly
                if language_name.lower() == LanguageNames.ARABIC_LEVANTINE.value.lower():
                    lines = multiline_text.split('\n')
                    multiline_text = '\n'.join(reversed(lines))

                annotate_image_with_subtitles(config=config,
                                              image_annotator=image_annotator,
                                              multiline_text=multiline_text,
                                              multiline_y_start=multiline_y_start,
                                              romanized_multiline_text=romanized_multiline_text)

            image = write_frame_to_video_file(pil_image=pil_image,
                                              video_writer=video_writer)

            if show_while_annotating:
                max_length = 720
                if max(image.shape[:2]) > max_length:
                    scale_factor = max_length / max(image.shape[:2])
                    display_image = cv2.resize(image, (0, 0), fx=scale_factor, fy=scale_factor)
                else:
                    display_image = image
                cv2.imshow(str(Path(video_path).stem), display_image)
                if cv2.waitKey(1) & 0xFF == ord('q'):
                    break
    finally:
        video_reader.release()
        video_writer.release()
        finish_video_and_attach_audio_from_original(original_video_path=video_path,
                                                    no_audio_video_path=no_audio_video_path,
                                                    subtitled_video_path=subtitled_video_path)
        cv2.destroyAllWindows()


def annotate_image_with_subtitles(config: LanguageAnnotationConfig,
                                  image_annotator: ImageDraw,
                                  multiline_text: str,
                                  multiline_y_start: int,
                                  romanized_multiline_text: str = None
                                  ) -> None:
    number_of_lines = multiline_text.count('\n') + 1

    # Annotate the frame with the current segment using PIL
    image_annotator.multiline_text(xy=(config.buffer_size, multiline_y_start),
                                   text=multiline_text,
                                   fill=config.color,
                                   stroke_width=3,
                                   stroke_fill=(0, 0, 0),
                                   font=config.language_font)
    if romanized_multiline_text is not None:

        image_annotator.multiline_text(
            xy=(config.buffer_size, multiline_y_start + config.language_font.size * number_of_lines * 1.5),
            text=romanized_multiline_text,
            fill=config.color,
            stroke_width=3,
            stroke_fill=(0, 0, 0),
            font=config.language_font)


def highlight_current_word(current_segment: TranslatedTranscriptSegmentWithWords,
                           current_word: TranslatedWhisperWordTimestamp) -> dict[str, dict[str, str]]:
    """Highlight the current word in the segment text, ignoring punctuation."""

    # Define a helper function to strip punctuation
    def strip_punctuation(text: str) -> str:
        return re.sub(r'[^\w\s]', '', text)

    highlighted_segments = {}
    for language_name in current_segment.og_text_and_translations.keys():
        # if language_name.lower() in LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
        #     # Use jieba cut words for highlighting
        #     segment_words = list(jieba.cut(current_segment))
        #     stripped_current_word_text = strip_punctuation(current_word_text)
        #     for current_word in segment_words:
        #         stripped_word = strip_punctuation(current_word)
        #         if stripped_word in stripped_current_word_text:
        #             highlighted_segment_words.append(f"[{current_word}]")
        #         else:
        #             highlighted_segment_words.append(current_word)
        # elif language_name.lower() == LanguageNames.ARABIC_LEVANTINE.value.lower():
        #     # Use space-split words for Arabic text
        #     segment_words = segment_text_display.split()
        #     stripped_current_word_text = strip_punctuation(current_word_text)
        #     for current_word in segment_words:
        #         stripped_word = strip_punctuation(current_word)
        #         if stripped_word in stripped_current_word_text:
        #             highlighted_segment_words.append(f"[{current_word}]")
        #         else:
        #             highlighted_segment_words.append(current_word)
        # else:

        # Use space-split words for other languages
        segment_text, romanized_text = current_segment.get_text_by_language(language_name)
        current_word_text, romanized_current_word_text = current_word.get_word_by_language(language_name)

        stripped_current_word_text = strip_punctuation(current_word_text)
        segment_words = segment_text.split()

        highlighted_words = []
        for segment_word in segment_words:
            if stripped_current_word_text == strip_punctuation(segment_word):
                highlighted_words.append(f"[{segment_word}]")
            else:
                highlighted_words.append(segment_word)
        highlighted_segments[language_name] = {}
        highlighted_segments[language_name]['text'] = " ".join(highlighted_words)

        if romanized_text is not None:
            stripped_romanized_word_text = strip_punctuation(romanized_current_word_text)
            romanized_words = romanized_text.split()
            highlighted_words = []
            for romanized_word in romanized_words:
                if stripped_romanized_word_text == strip_punctuation(romanized_word):
                    highlighted_words.append(f"[{romanized_word}]")
                else:
                    highlighted_words.append(romanized_word)
            highlighted_segments[language_name]['romanized'] = " ".join(highlighted_words)

    return highlighted_segments
