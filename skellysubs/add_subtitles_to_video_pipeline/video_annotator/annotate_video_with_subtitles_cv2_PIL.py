import logging
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

import cv2
from PIL import Image, ImageFont, ImageDraw
from arabic_reshaper import arabic_reshaper
from bidi.algorithm import get_display
from tqdm import tqdm

from skellysubs.add_subtitles_to_video_pipeline.video_annotator.video_reader_writer_methods import \
    create_video_reader_and_writer, write_frame_to_video_file, finish_video_and_attach_audio_from_original
from skellysubs.translate_transcript_pipeline.models.language_models import LanguageNames
from skellysubs.translate_transcript_pipeline.models.translated_transcript_model import \
    TranslatedTranscription, MatchedTranslatedSegment, MatchedTranslatedWord, TranslatedTranscriptSegmentWithWords

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

            current_segment_and_matched_word = translated_transcript.get_matched_segment_and_word_at_timestamp(
                frame_timestamp)
            current_segment = current_segment_and_matched_word.current_segment
            for language_name, config in LANGUAGE_ANNOTATION_CONFIGS.items():
                current_matched_word = current_segment_and_matched_word.matched_word_by_language[language_name]

                multiline_y_start = config.language_start_y(video_height)

                #
                #
                # # Convert multiline text to list of tuples for annotation
                # multiline_text_tuples = [
                #     (word, is_highlighted) for word, is_highlighted in highlighted_segment_text['text']
                # ]
                # romanized_text_tuples = [
                #     (word, is_highlighted) for word, is_highlighted in highlighted_segment_text['romanized']
                # ] if highlighted_segment_text.get('romanized') else None

                annotate_image_with_subtitles(config=config,
                                              image_annotator=image_annotator,
                                              multiline_y_start=multiline_y_start,
                                              current_segment=current_segment,
                                              current_matched_word=current_matched_word,
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
    except Exception as e:
        logger.error(f"Error while annotating video: {e}")
        raise e
    finally:
        video_reader.release()
        video_writer.release()
        finish_video_and_attach_audio_from_original(original_video_path=video_path,
                                                    no_audio_video_path=no_audio_video_path,
                                                    subtitled_video_path=subtitled_video_path)
        cv2.destroyAllWindows()


def annotate_image_with_subtitles(config: LanguageAnnotationConfig,
                                  image_annotator: ImageDraw,
                                  current_segment: TranslatedTranscriptSegmentWithWords,
                                  current_matched_word: MatchedTranslatedWord,
                                  multiline_y_start: int,
                                  video_width: int,
                                  video_height: int,
                                  ) -> None:
    right_to_left: bool = config.language_name.lower() in LanguageNames.ARABIC_LEVANTINE.value.lower()
    current_y =  multiline_y_start
    if right_to_left:
        current_x = video_width - config.buffer_size*2
    else:
        current_x  = config.buffer_size

    translated_words_list, romanized_words_list = current_segment.get_word_list_by_language(config.language_name)

    if config.language_name.lower() in LanguageNames.ARABIC_LEVANTINE.value.lower():
        translated_words_list = [get_display(arabic_reshaper.reshape(word)) for word in translated_words_list]

    for word_type, words_list in zip(('translated', 'romanized'), [translated_words_list, romanized_words_list]):
        if not words_list:
            continue
        romanized= word_type == 'romanized'
        for word_number, word in enumerate(words_list):

            _, _, text_width, text_height = config.language_font.getbbox(word + " ")
            if word_number == current_matched_word.translated_word_index:
                # if right_to_left and not romanized:
                #     rectangle_l_b_w_h = [current_x - text_width - 5,
                #                          current_y - 5,
                #                          current_x + 5,
                #                          current_y + text_height + 5]
                # else:
                #     rectangle_l_b_w_h = [current_x - 5,
                #                          current_y - 5,
                #                          current_x + text_width + 5,
                #                          current_y + text_height + 5]
                # image_annotator.rectangle(
                #     rectangle_l_b_w_h,
                #     fill=(255, 0, 255, 155),  # Add transparency
                #     outline=(0, 0, 0)
                # )

                image_annotator.text((current_x if not right_to_left and not romanized else current_x - text_width,
                                      current_y),
                                     text=word,
                                     fill=config.color,
                                     font=config.language_font,
                                     stroke_width=8,
                                     stroke_fill=(0, 0, 255),
                                     align="left"
                                     )


            #annotate the word regardless of whether it is the matches
            image_annotator.text((current_x if not right_to_left and not romanized else current_x - text_width,
                                  current_y),
                                     text=word,
                                     fill=config.color,
                                     font=config.language_font,
                                     stroke_width=2,
                                     stroke_fill=(0, 0, 0),
                                     align="left"
                                     )


            if right_to_left and not romanized:
                if current_x - text_width < config.buffer_size:
                    current_y += text_height
                    current_x = video_width - config.buffer_size*2
                else:
                    current_x -= text_width

            else:
                if current_x + text_width > video_width - config.buffer_size:
                    current_y += text_height
                    current_x = config.buffer_size
                else:
                    current_x += text_width


