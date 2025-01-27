import enum
from typing import Callable

from skellysubs.ai_clients.ai_client_abc import AiClientABC
from skellysubs.ai_clients.ollama_client import OllamaClient
from skellysubs.ai_clients.openai_client import OpenaiClient

def get_ai_client(ai_client_strategy: str="openai") -> AiClientABC:
    return AiClientStrategies.OPENAI.value if ai_client_strategy == "openai" else AiClientStrategies.OLLAMA.value

class AiClientStrategies(enum.Enum):
    OPENAI = OpenaiClient()
    OLLAMA = OllamaClient()
