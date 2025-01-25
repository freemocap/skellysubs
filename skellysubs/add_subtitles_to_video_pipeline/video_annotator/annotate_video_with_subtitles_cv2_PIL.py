import logging
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

import cv2
import jieba
from PIL import Image, ImageFont, ImageDraw
from PIL.ImageFont import truetype
from arabic_reshaper import arabic_reshaper
from bidi.algorithm import get_display
from tqdm import tqdm

from skellysubs.add_subtitles_to_video_pipeline.video_annotator.create_multi_line_text import \
    create_multiline_text_chinese, create_multiline_text
from skellysubs.add_subtitles_to_video_pipeline.video_annotator.video_reader_writer_methods import \
    create_video_reader_and_writer, write_frame_to_video_file, finish_video_and_attach_audio_from_original
from skellysubs.translate_transcript_pipeline.models.language_models import LanguageNames
from skellysubs.translate_transcript_pipeline.models.translated_transcript_model import \
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
                                                             font_path=str(FONT_BASE_PATH / "ARIAL.TTF"),
                                                             color=(231, 41, 138),
                                                             font_size=48,
                                                             language_start_y=lambda video_height: int(
                                                                 video_height // 1.35),
                                                             buffer_size=100),
}


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
            for language_name, config in LANGUAGE_ANNOTATION_CONFIGS.items():

                highlighted_segment_text = highlight_current_word_in_segment_texts(
                    current_segment=current_segment,
                    current_word=current_word,
                    language_name=language_name)

                multiline_y_start = config.language_start_y(video_height)


                # Arabic text reshaping and display
                if language_name.lower() == LanguageNames.ARABIC_LEVANTINE.value.lower():
                    reshaped_words = [
                        (get_display(arabic_reshaper.reshape(word)), is_highlighted)
                        for word, is_highlighted in highlighted_segment_text['text']
                    ]
                    highlighted_segment_text['text'] = reshaped_words

                # Prepare multiline text
                if language_name.lower() == LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
                    multiline_text = create_multiline_text_chinese(
                        " ".join(word for word, _ in highlighted_segment_text['text']),
                        config.language_font,
                        video_width,
                        config.buffer_size
                    )
                else:
                    multiline_text = create_multiline_text(
                        " ".join(word for word, _ in highlighted_segment_text['text']),
                        config.language_font,
                        video_width,
                        config.buffer_size
                    )


                romanized_multiline_text = None
                if highlighted_segment_text.get('romanized'):
                    romanized_segment_text = " ".join(word for word, _ in highlighted_segment_text['romanized'])
                    romanized_multiline_text = create_multiline_text(
                        romanized_segment_text,
                        config.language_font,
                        video_width,
                        config.buffer_size
                    )

                # Convert multiline text to list of tuples for annotation
                multiline_text_tuples = [
                    (word, is_highlighted) for word, is_highlighted in highlighted_segment_text['text']
                ]
                romanized_text_tuples = [
                    (word, is_highlighted) for word, is_highlighted in highlighted_segment_text['romanized']
                ] if highlighted_segment_text.get('romanized') else None

                # Reverse lines for Arabic text to render correctly
                if language_name.lower() == LanguageNames.ARABIC_LEVANTINE.value.lower():
                    multiline_text_tuples = list(reversed(multiline_text_tuples))


                annotate_image_with_subtitles(config=config,
                                              image_annotator=image_annotator,
                                                multiline_text_tuples=multiline_text_tuples,
                                              multiline_y_start=multiline_y_start,
                                              romanized_multiline_text_tuples=romanized_text_tuples,
                                                video_width=video_width,
                                                video_height=video_height)

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


def highlight_current_word_in_segment_texts(current_segment: TranslatedTranscriptSegmentWithWords,
                                            current_word: TranslatedWhisperWordTimestamp,
                                            language_name: LanguageNames) -> dict[str, list[tuple[str, bool]]]:
    """Highlight the current word in the segment text, ignoring punctuation."""

    def strip_punctuation(text: str) -> str:
        return re.sub(r'[^\w\s]', '', text)

    highlighted_segments = {}

    segment_text, romanized_text = current_segment.get_text_by_language(language_name)
    current_word_text, romanized_current_word_text = current_word.get_word_by_language(language_name)

    stripped_current_word_text = strip_punctuation(current_word_text)
    if language_name.lower() in LanguageNames.CHINESE_MANDARIN_SIMPLIFIED.value.lower():
        segment_words = list(jieba.cut(segment_text))
    else:
        segment_words = segment_text.split()

    highlighted_words = []

    for segment_word in segment_words:
        if stripped_current_word_text and stripped_current_word_text in strip_punctuation(
                segment_word) or strip_punctuation(current_word_text) in strip_punctuation(segment_word):
            highlighted_words.append((segment_word, True))
        else:
            highlighted_words.append((segment_word, False))

    highlighted_segments['text'] = highlighted_words

    if romanized_text is not None:
        stripped_romanized_word_text = strip_punctuation(romanized_current_word_text)
        romanized_words = romanized_text.split()

        highlighted_romanized_words = []
        for romanized_word in romanized_words:
            if stripped_romanized_word_text and stripped_romanized_word_text in strip_punctuation(
                    romanized_word) or strip_punctuation(romanized_current_word_text) in strip_punctuation(
                    romanized_word):
                highlighted_romanized_words.append((romanized_word, True))
            else:
                highlighted_romanized_words.append((romanized_word, False))

    return highlighted_segments


def annotate_image_with_subtitles(config: LanguageAnnotationConfig,
                                  image_annotator: ImageDraw,
                                  multiline_text_tuples: list[tuple[str, bool]],
                                  multiline_y_start: int,
                                  video_width: int,
                                  video_height: int,
                                  romanized_multiline_text_tuples: list[tuple[str, bool]] = None) -> None:
    current_y = multiline_y_start
    current_x = config.buffer_size

    for word, is_highlighted in multiline_text_tuples:

        _, _, text_width, text_height = config.language_font.getbbox(word)
        if is_highlighted:
            image_annotator.rectangle(
                [current_x, current_y, current_x + text_width, current_y + text_height],
                fill=(255,0,255),  # Add transparency
                outline=config.color
            )
        image_annotator.text(xy=(current_x, current_y),
                             text=word,
                             fill=config.color,
                             font=config.language_font,
                             stroke_width=2,
                             stroke_fill=(0, 0, 0),
                             align="left" if config.language_name != LanguageNames.ARABIC_LEVANTINE else "right"
                             )
        if current_x + text_width > video_width - config.buffer_size:
            current_y += text_height + 5  # Add some space between lines
            current_x = config.buffer_size
        else:
            current_x += text_width + 5  # Add some space between words

    if romanized_multiline_text_tuples is not None:
        current_y += config.language_font.size * 1.5
        for word, is_highlighted in romanized_multiline_text_tuples:
            text_width, text_height = image_annotator.textsize(word, font=config.language_font)
            if is_highlighted:
                image_annotator.rectangle(
                    [config.buffer_size, current_y, config.buffer_size + text_width, current_y + text_height],
                    fill=config.color + (50,),  # Add transparency
                    outline=config.color
                )
            image_annotator.text((config.buffer_size, current_y), word, fill=config.color, font=config.language_font)
            current_y += text_height + 5  # Add some space between lines
