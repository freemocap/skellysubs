import yaml
from typing import Dict, List

from PIL import ImageFont
from pydantic import BaseModel, ConfigDict
from pathlib import Path

from skellysubs.core.translation_pipeline.language_configs.language_configs import FrozenModel, _load_language_configs
from skellysubs.core.translation_pipeline.models.translation_typehints import LanguageNameString



FONT_BASE_PATH = Path(__file__).parent.parent.parent.parent.parent / 'fonts'
if not FONT_BASE_PATH.exists():
    raise ValueError(f"Font base path `{FONT_BASE_PATH}` does not exist")


DEFAULT_FONT_PATH = FONT_BASE_PATH / "ARIAL.TTF"
if not DEFAULT_FONT_PATH.exists():
    raise ValueError(f"Default font path `{DEFAULT_FONT_PATH}` does not exist")

DEFAULT_FONT_SIZE_RATIO = 0.018

def get_default_text_height(image_height: int) -> int:
    return int(image_height * DEFAULT_FONT_SIZE_RATIO)

def get_default_font(image_height: int) -> ImageFont:
    return ImageFont.truetype(str(DEFAULT_FONT_PATH), get_default_text_height(image_height))

class LanguageAnnotationConfig(FrozenModel):
    font_file: str #  should be a file in the FONT_BASE_PATH directory
    font_size_ratio: float = 1
    buffer_size: int
    color: tuple[int, int, int]

    def get_font_size(self, image_height: int) -> int:
        return int(self.font_size_ratio * DEFAULT_FONT_SIZE_RATIO * image_height)

    def get_font(self, image_height: int) -> ImageFont:
        font_path = Path(FONT_BASE_PATH)/self.font_file
        if not font_path.exists():
            raise ValueError(f"Font definition file not found at: {font_path}")
        if not font_path.is_file():
            raise ValueError(f"Font definition path is not a file: {font_path}")
        if not font_path.suffix.lower() in ['.ttf', '.otf']:
            raise ValueError(f"Font file `{font_path}` is not a valid font file - suffix must be .ttf or .otf, received: {font_path.suffix}")
        return ImageFont.truetype(str(font_path), self.get_font_size(image_height))




def get_annotation_configs() -> Dict[LanguageNameString, LanguageAnnotationConfig]:
    global LANGUAGE_CONFIGS
    global LANGUAGE_ANNOTATION_CONFIGS
    if LANGUAGE_CONFIGS is None:
        _, LANGUAGE_ANNOTATION_CONFIGS = _load_language_configs()
    return LANGUAGE_ANNOTATION_CONFIGS

if __name__ == "__main__":
    import json
    outer_annotation_configs = get_annotation_configs()
    annotation_configs_as_dict = {lang_name: lang_config.model_dump() for lang_name, lang_config in outer_annotation_configs.items()}
    print(json.dumps(annotation_configs_as_dict, indent=2))