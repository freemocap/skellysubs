import yaml
from typing import Dict, List

from PIL import ImageFont
from pydantic import BaseModel, ConfigDict
from pathlib import Path

from skellysubs.core.translation_pipeline.models.translation_typehints import LanguageNameString


class FrozenModel(BaseModel):
    model_config = ConfigDict(frozen=True)

class LanguageBackground(FrozenModel):
    family_tree: List[str]
    alphabet: str
    sample_text: str

FONT_BASE_PATH = Path(__file__).parent.parent.parent.parent.parent / 'fonts'
if not FONT_BASE_PATH.exists():
    raise ValueError(f"Font base path `{FONT_BASE_PATH}` does not exist")


DEFAULT_FONT_PATH = FONT_BASE_PATH / "ARIAL.TTF"
if not DEFAULT_FONT_PATH.exists():
    raise ValueError(f"Default font path `{DEFAULT_FONT_PATH}` does not exist")

DEFAULT_FONT_SIZE_RATIO = 0.016

def get_default_text_height(image_height: int) -> int:
    return int(image_height * DEFAULT_FONT_SIZE_RATIO)

def get_default_font(image_height: int) -> ImageFont:
    return ImageFont.truetype(str(DEFAULT_FONT_PATH), get_default_text_height(image_height))

class LanguageAnnotationConfig(BaseModel):
    font_file: str #  should be a file in the FONT_BASE_PATH directory
    font_size_ratio: float
    buffer_size: int
    color: tuple[int, int, int]

    def get_font_size(self, image_height: int) -> int:
        return int(self.font_size_ratio * image_height)

    def get_font(self, image_height: int) -> ImageFont:
        font_path = Path(FONT_BASE_PATH)/self.font_file
        if not font_path.exists():
            raise ValueError(f"Font definition file not found at: {font_path}")
        if not font_path.is_file():
            raise ValueError(f"Font definition path is not a file: {font_path}")
        if not font_path.suffix.lower() in ['.ttf', '.otf']:
            raise ValueError(f"Font file `{font_path}` is not a valid font file - suffix must be .ttf or .otf, received: {font_path.suffix}")
        return ImageFont.truetype(str(font_path), self.get_font_size(image_height))



class LanguageConfig(FrozenModel):
    language_name: LanguageNameString
    language_code: str
    romanization_method: str
    background: LanguageBackground

    @classmethod
    def from_config(cls, config: dict):
        return cls(**config)

def _load_language_configs(yaml_path: str | None=None) ->tuple[dict[LanguageNameString, LanguageConfig], dict[LanguageNameString, LanguageAnnotationConfig]]:
    if not yaml_path:
        yaml_path = Path(__file__).parent / 'language_configs.yaml'
    with open(yaml_path, 'r', encoding="utf-8") as file:
        config_data = yaml.safe_load(file)
        language_configs = {name.lower(): LanguageConfig(**config) for name, config in config_data['language_configs'].items()}
        annotation_configs = {name.lower(): LanguageAnnotationConfig(**config) for name, config in config_data['annotation_configs'].items()}
    if not language_configs or not annotation_configs:
        raise ValueError("No language configs found in the yaml file")
    if not language_configs.keys() == annotation_configs.keys():
        raise ValueError("Language configs and annotation configs do not match")
    return language_configs, annotation_configs

LANGUAGE_CONFIGS: dict[LanguageNameString, LanguageConfig]| None = None
LANGUAGE_ANNOTATION_CONFIGS: dict[LanguageNameString, LanguageAnnotationConfig] | None = None
def get_language_configs() -> Dict[LanguageNameString, LanguageConfig]:
    global LANGUAGE_CONFIGS
    global LANGUAGE_ANNOTATION_CONFIGS
    if LANGUAGE_CONFIGS is None:
        LANGUAGE_CONFIGS, LANGUAGE_ANNOTATION_CONFIGS = _load_language_configs()
    return LANGUAGE_CONFIGS

def get_annotation_configs() -> Dict[LanguageNameString, LanguageAnnotationConfig]:
    global LANGUAGE_CONFIGS
    global LANGUAGE_ANNOTATION_CONFIGS
    if LANGUAGE_CONFIGS is None:
        _, LANGUAGE_ANNOTATION_CONFIGS = _load_language_configs()
    return LANGUAGE_ANNOTATION_CONFIGS

if __name__ == "__main__":
    import json
    outer_language_configs = get_language_configs()
    language_configs_as_dict = {lang_name: lang_config.model_dump() for lang_name, lang_config in outer_language_configs.items()}
    print(json.dumps(language_configs_as_dict, indent=2))
    outer_annotation_configs = get_annotation_configs()
    annotation_configs_as_dict = {lang_name: lang_config.model_dump() for lang_name, lang_config in outer_annotation_configs.items()}
    print(json.dumps(annotation_configs_as_dict, indent=2))