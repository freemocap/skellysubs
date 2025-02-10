import yaml
from typing import Dict, List

from PIL import ImageFont
from pydantic import BaseModel, ConfigDict
from pathlib import Path

from skellysubs.skellysubs_core.translation_pipeline.models.translation_typehints import LanguageNameString


class FrozenModel(BaseModel):
    model_config = ConfigDict(frozen=True)

class LanguageBackground(FrozenModel):
    family_tree: List[str]
    alphabet: str
    sample_text: str


class LanguageAnnotationConfig(BaseModel):
    font_path: str
    font_size_ratio: float
    buffer_size: int
    color: tuple[int, int, int]

    def get_font_size(self, image_height: int) -> int:
        return int(self.font_size_ratio * image_height)

    def get_font(self, image_height: int) -> ImageFont:
        return ImageFont.truetype(self.font_path, self.get_font_size(image_height))



class LanguageConfig(FrozenModel):
    language_name: LanguageNameString
    language_code: str
    romanization_method: str
    annotation_config: LanguageAnnotationConfig
    background: LanguageBackground

    @classmethod
    def from_config(cls, config: dict):
        return cls(**config)

def load_language_configs(yaml_path: str|None=None) -> Dict[str, LanguageConfig]:
    if not yaml_path:
        yaml_path = Path(__file__).parent / 'language_configs.yaml'
    with open(yaml_path, 'r', encoding="utf-8") as file:
        config_data = yaml.safe_load(file)
        languages = config_data['languages']
        return {lang_name: LanguageConfig(**lang_data) for lang_name, lang_data in languages.items()}

LANGUAGE_CONFIGS: Dict[LanguageNameString, LanguageConfig]| None = None
def get_language_configs() -> Dict[LanguageNameString, LanguageConfig]:
    global LANGUAGE_CONFIGS
    if not LANGUAGE_CONFIGS:
        LANGUAGE_CONFIGS = load_language_configs()
    return LANGUAGE_CONFIGS
get_language_configs() # to initialize the global variable on initialization
if __name__ == "__main__":
    import json
    language_configs = load_language_configs()
    language_configs_as_dict = {lang_name: lang_config.model_dump() for lang_name, lang_config in language_configs.items()}
    print(json.dumps(language_configs_as_dict, indent=2))