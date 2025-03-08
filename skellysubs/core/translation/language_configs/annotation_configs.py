from pathlib import Path

import yaml
from PIL import ImageFont

from skellysubs.core.translation.models.translation_typehints import LanguageNameString
from skellysubs.utilities.frozen_model import FrozenModel

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
    font_file: str  # should be a file in the FONT_BASE_PATH directory
    font_size_ratio: float = 1
    buffer_size: int
    color: tuple[int, int, int]

    def get_font_size(self, image_height: int) -> int:
        return int(self.font_size_ratio * DEFAULT_FONT_SIZE_RATIO * image_height)

    def get_font(self, image_height: int) -> ImageFont:
        font_path = Path(FONT_BASE_PATH) / self.font_file
        if not font_path.exists():
            raise ValueError(f"Font definition file not found at: {font_path}")
        if not font_path.is_file():
            raise ValueError(f"Font definition path is not a file: {font_path}")
        if not font_path.suffix.lower() in ['.ttf', '.otf']:
            raise ValueError(
                f"Font file `{font_path}` is not a valid font file - suffix must be .ttf or .otf, received: {font_path.suffix}")
        return ImageFont.truetype(str(font_path), self.get_font_size(image_height))


def get_annotation_configs(yaml_path: str | None = None) -> dict[LanguageNameString, LanguageAnnotationConfig]:
    if not yaml_path:
        yaml_path = Path(__file__).parent / 'language_configs.yaml'
    with open(yaml_path, 'r', encoding="utf-8") as file:
        config_data = yaml.safe_load(file)
        language_configs = {name.lower(): LanguageAnnotationConfig(**config) for name, config in
                            config_data['language_configs'].items()}
        annotation_configs = {name.lower(): LanguageAnnotationConfig(**config) for name, config in
                              config_data['annotation_configs'].items()}
    if not language_configs or not annotation_configs:
        raise ValueError("No language configs found in the yaml file")
    if not language_configs.keys() == annotation_configs.keys():
        raise ValueError("Language configs and annotation configs do not match")
    return annotation_configs


if __name__ == "__main__":
    import json

    outer_annotation_configs = get_annotation_configs()
    annotation_configs_as_dict = {lang_name: lang_config.model_dump() for lang_name, lang_config in
                                  outer_annotation_configs.items()}
    print(json.dumps(annotation_configs_as_dict, indent=2))
