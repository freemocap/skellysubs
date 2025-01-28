from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from PIL import ImageFont

from skellysubs.translate_transcript_pipeline.models.language_models import LanguageNames


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

DEFAULT_FONT_SIZE = 48
FONT_BASE_PATH = Path(__file__).parent.parent.parent.parent / "fonts"
DEFAULT_FONT = ImageFont.truetype(str(FONT_BASE_PATH / "ARIAL.TTF"), DEFAULT_FONT_SIZE)
LANGUAGE_ANNOTATION_CONFIGS = {
    LanguageNames.ENGLISH: LanguageAnnotationConfig(language_name=LanguageNames.ENGLISH,
                                                    font_path=str(FONT_BASE_PATH / "ARIAL.TTF"),
                                                    color=(27, 158, 119),
                                                    font_size=DEFAULT_FONT_SIZE,
                                                    language_start_y=lambda video_height: 50,
                                                    buffer_size=100),
    LanguageNames.SPANISH: LanguageAnnotationConfig(language_name=LanguageNames.SPANISH,
                                                    font_path=str(FONT_BASE_PATH / "ARIAL.TTF"),
                                                    font_size=DEFAULT_FONT_SIZE,
                                                    color=(217, 95, 2),
                                                    language_start_y=lambda video_height: int(video_height // 6),
                                                    buffer_size=100),
    LanguageNames.CHINESE_MANDARIN_SIMPLIFIED: LanguageAnnotationConfig(
        language_name=LanguageNames.CHINESE_MANDARIN_SIMPLIFIED,
        font_path=str(FONT_BASE_PATH / "NotoSerifCJKsc-VF-Simplified-Chinese.ttf"),
        font_size=int(DEFAULT_FONT_SIZE * 1.2),
        color=(157, 152, 219),
        language_start_y=lambda video_height: int(video_height //2),
        buffer_size=100),
    LanguageNames.ARABIC_LEVANTINE: LanguageAnnotationConfig(language_name=LanguageNames.ARABIC_LEVANTINE,
                                                             font_path=str(FONT_BASE_PATH / "ARIAL.TTF"),
                                                             color=(231, 41, 138),
                                                             font_size=int(DEFAULT_FONT_SIZE*1.15),
                                                             language_start_y=lambda video_height: int(
                                                                 video_height // 1.35),
                                                             buffer_size=100),
}
