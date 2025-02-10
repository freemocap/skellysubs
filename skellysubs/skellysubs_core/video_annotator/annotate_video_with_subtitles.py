import logging
from pathlib import Path

import cv2
from PIL import Image, ImageDraw
from arabic_reshaper import arabic_reshaper
from bidi.algorithm import get_display
from tqdm import tqdm

from skellysubs.skellysubs_core.video_annotator.language_annotation_configs import \
    LanguageAnnotationConfig, LANGUAGE_ANNOTATION_CONFIGS, get_default_font
from skellysubs.skellysubs_core.video_annotator.video_reader_writer_methods import \
    create_video_reader_and_writer, write_frame_to_video_file, finish_video_and_attach_audio_from_original
from skellysubs.skellysubs_core.translation_pipeline.models.language_models import LanguageNames
from skellysubs.skellysubs_core.translation_pipeline.models.translated_transcript_model import \
    TranslatedTranscription
from skellysubs.skellysubs_core.translation_pipeline.models.translated_segment_models import MatchedTranslatedWord, \
    TranscriptSegment

logger = logging.getLogger(__name__)


def annotate_video_with_subtitles(video_path: str,
                                  translated_transcript: TranslatedTranscription,
                                  subtitled_video_path: str,
                                  show_while_annotating: bool = True,
                                  transpose_for_vertical_video: bool = True
                                  ) -> None:
    (no_audio_video_path,
     video_height,
     video_reader,
     video_width,
     video_writer) = create_video_reader_and_writer(
        og_video_path=video_path,
        subtitled_video_output_path=subtitled_video_path,
        transpose_for_vertical_video=transpose_for_vertical_video
    )

    video_number_of_frames = int(video_reader.get(cv2.CAP_PROP_FRAME_COUNT))
    try:
        # Go through each frame of the video and annotate it with the translated words based on their timestamps
        for frame_number in tqdm(range(video_number_of_frames),
                                 desc="Annotating video with subtitles",
                                 total=video_number_of_frames):
            if not video_reader.isOpened():
                raise ValueError(f"Video reader is not open: {video_path}")

            read_success, image = video_reader.read()

            # image = cv2.rotate(image, cv2.ROTATE_90_COUNTERCLOCKWISE    )
            if not read_success or not video_reader.isOpened():
                break
            image = cv2.cvtColor(image, cv2.COLOR_BGR2RGB)
            frame_number += 1

            frame_timestamp = video_reader.get(
                cv2.CAP_PROP_POS_MSEC) / 1000  # seconds - internally based on frame# times  *presumed* frame duration based on specified framerate
            pil_image = Image.fromarray(image)
            image_annotator = ImageDraw.Draw(pil_image)

            current_segment_and_matched_word = translated_transcript.get_matched_segment_and_word_at_timestamp(
                frame_timestamp)

            current_segment = current_segment_and_matched_word.current_segment
            for language_name, config in LANGUAGE_ANNOTATION_CONFIGS.items():
                current_matched_word = current_segment_and_matched_word.matched_word_by_language[language_name]

                multiline_y_start = config.language_start_y(video_height)

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
                                  current_segment: TranscriptSegment,
                                  current_matched_word: MatchedTranslatedWord,
                                  multiline_y_start: int,
                                  video_width: int,
                                  video_height: int,
                                  ) -> None:
    right_to_left: bool = config.language_name.lower() in LanguageNames.ARABIC_LEVANTINE.value.lower()
    current_y = multiline_y_start
    if right_to_left:
        current_x = video_width - config.buffer_size
    else:
        current_x = config.buffer_size

    translated_words_list, romanized_words_list = current_segment.get_word_list_by_language(config.language_name)

    if config.language_name.lower() in LanguageNames.ARABIC_LEVANTINE.value.lower():
        translated_words_list = [get_display(arabic_reshaper.reshape(word)) for word in translated_words_list]

    language_font = config.get_font(image_height=video_height)
    for word_type, words_list in zip(('translated', 'romanized'), [translated_words_list, romanized_words_list]):
        if not words_list:
            continue

        for word_number, word in enumerate(words_list):

            _, _, text_width, text_height = language_font.getbbox(word + " ")

            # initialize stuff romanization run
            if word_number == 0 and word_type == 'romanized':
                    language_font = get_default_font(image_height=video_height)
                    right_to_left = False
                    current_x = config.buffer_size
                    current_y += int(text_height * 1.75)

            # Check if should bump to next line
            if right_to_left:
                if current_x - text_width < config.buffer_size:
                    current_y += text_height
                    current_x = video_width - config.buffer_size

            else:
                if current_x + text_width > video_width - (config.buffer_size):
                    current_y += text_height
                    current_x = config.buffer_size

            #check if should highlight this word
            if word_number == current_matched_word.translated_word_index:
                image_annotator.text((current_x if not right_to_left else current_x - text_width,
                                      current_y),
                                     text=word,
                                     fill=config.color,
                                     font=language_font,
                                     stroke_width=14,
                                     stroke_fill=(0, 255, 0),
                                     align="left"
                                     )
                image_annotator.text((current_x if not right_to_left else current_x - text_width,
                                      current_y),
                                     text=word,
                                     fill=config.color,
                                     font=language_font,
                                     stroke_width=10,
                                     stroke_fill=(0, 0, 255),
                                     align="left"
                                     )

            # Standard text annotate the word happens each loop)
            image_annotator.text((current_x if not right_to_left else current_x - text_width,
                                  current_y),
                                 text=word,
                                 fill=config.color,
                                 font=language_font,
                                 stroke_width=6,
                                 stroke_fill=(11, 11, 11),
                                 align="left"
                                 )
            image_annotator.text((current_x if not right_to_left else current_x - text_width,
                                  current_y),
                                 text=word,
                                 fill=config.color,
                                 font=language_font,
                                 stroke_width=1,
                                 stroke_fill=config.color,
                                 align="left"
                                 )


            #increment x cursor position
            if right_to_left:
                current_x -= text_width
            else:
                current_x += text_width
