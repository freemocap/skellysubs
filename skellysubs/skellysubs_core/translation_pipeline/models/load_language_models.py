import yaml
from typing import Dict, List
from pydantic import BaseModel, ConfigDict
from pathlib import Path

class FrozenModel(BaseModel):
    model_config = ConfigDict(frozen=True)

class LanguageBackground(FrozenModel):
    family_tree: List[str]
    alphabet: str
    sample_text: str

class LanguageAnnotationConfig(FrozenModel):
    font_path: str
    font_size_ratio: float
    buffer_size: int
    color: tuple[int, int, int]
    language_start_y_offset: float

class LanguageConfig(FrozenModel):
    romanization_method: str
    annotation_config: LanguageAnnotationConfig
    background: LanguageBackground

def load_language_configs(yaml_path: Path) -> Dict[str, LanguageConfig]:
    with open(yaml_path, 'r', encoding="utf-8") as file:
        config_data = yaml.safe_load(file)
        languages = config_data['languages']
        return {lang_name: LanguageConfig(**lang_data) for lang_name, lang_data in languages.items()}

if __name__ == "__main__":
    import json
    yaml_path = Path('language_configs.yaml')
    language_configs = load_language_configs(yaml_path)
    language_configs_as_dict = {lang_name: lang_config.model_dump() for lang_name, lang_config in language_configs.items()}
    print(json.dumps(language_configs_as_dict, indent=2))