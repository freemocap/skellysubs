from pathlib import Path
from typing import Callable

from PIL import ImageFont
from pydantic import BaseModel

from skellysubs.translate_transcript_pipeline.models.language_models import LanguageNames


class LanguageAnnotationConfig(BaseModel):
    language_name: LanguageNames
    font_path: str
    font_size_ratio: float
    buffer_size: int
    color: tuple[int, int, int]
    language_start_y: Callable[[int], int]

    def get_font_size(self, image_height: int) -> int:
        return int(self.font_size_ratio * image_height)

    def get_font(self, image_height: int) -> ImageFont:
        return ImageFont.truetype(self.font_path, self.get_font_size(image_height))


SUBTITLES_TOP_BUFFER_RATIO = 0.55
SUBTITLES_BOTTOM_BUFFER_RATIO = 0.1
DEFAULT_FONT_SIZE_RATIO = 0.016

FONT_BASE_PATH = Path(__file__).parent.parent.parent / "fonts"


def get_default_font(image_height: int) -> ImageFont:
    return ImageFont.truetype(str(FONT_BASE_PATH / "ARIAL.TTF"), int(image_height * DEFAULT_FONT_SIZE_RATIO))


LANGUAGE_ANNOTATION_CONFIGS = {
    LanguageNames.ENGLISH: LanguageAnnotationConfig(language_name=LanguageNames.ENGLISH,
                                                    font_path=str(FONT_BASE_PATH / "ARIAL.TTF"),
                                                    color=(27, 158, 119),
                                                    font_size_ratio=DEFAULT_FONT_SIZE_RATIO,
                                                    language_start_y=lambda video_height: int(
                                                        video_height * SUBTITLES_TOP_BUFFER_RATIO),
                                                    buffer_size=100),
    LanguageNames.SPANISH: LanguageAnnotationConfig(language_name=LanguageNames.SPANISH,
                                                    font_path=str(FONT_BASE_PATH / "ARIAL.TTF"),
                                                    font_size_ratio=DEFAULT_FONT_SIZE_RATIO,
                                                    color=(217, 95, 2),
                                                    language_start_y=lambda video_height: int(
                                                        video_height * SUBTITLES_TOP_BUFFER_RATIO) + int(
                                                        video_height * 0.08),
                                                    buffer_size=100),

    LanguageNames.ARABIC_LEVANTINE: LanguageAnnotationConfig(language_name=LanguageNames.ARABIC_LEVANTINE,
                                                             font_path=str(FONT_BASE_PATH / "ARIAL.TTF"),
                                                             color=(231, 41, 138),
                                                             font_size_ratio=DEFAULT_FONT_SIZE_RATIO * 1.15,
                                                             language_start_y=lambda video_height: int(
                                                                 video_height * SUBTITLES_TOP_BUFFER_RATIO) + int(
                                                                 video_height * 0.15),
                                                             buffer_size=100),
    LanguageNames.CHINESE_MANDARIN_SIMPLIFIED: LanguageAnnotationConfig(
        language_name=LanguageNames.CHINESE_MANDARIN_SIMPLIFIED,
        font_path=str(FONT_BASE_PATH / "NotoSerifCJKsc-VF-Simplified-Chinese.ttf"),
        font_size_ratio=DEFAULT_FONT_SIZE_RATIO * 1.05,
        color=(157, 152, 219),
        language_start_y=lambda video_height: int(video_height * SUBTITLES_TOP_BUFFER_RATIO) + int(video_height * 0.26),
        buffer_size=100),
}
