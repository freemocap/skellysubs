from pathlib import Path
from typing import Dict, List

import yaml
from pydantic import BaseModel, ConfigDict

from skellysubs.core.translation_pipeline.language_configs.annotation_configs import LanguageAnnotationConfig
from skellysubs.core.translation_pipeline.models.translation_typehints import LanguageNameString


class FrozenModel(BaseModel):
    model_config = ConfigDict(frozen=True)

class LanguageBackground(FrozenModel):
    family_tree: List[str]
    alphabet: str
    sample_text: str


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



if __name__ == "__main__":
    import json
    outer_language_configs = get_language_configs()
    language_configs_as_dict = {lang_name: lang_config.model_dump() for lang_name, lang_config in outer_language_configs.items()}
    print(json.dumps(language_configs_as_dict, indent=2))
