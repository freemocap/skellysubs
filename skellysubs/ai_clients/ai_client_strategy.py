import enum

from skellysubs.ai_clients.ai_client_abc import AiClientABC
from skellysubs.ai_clients.ollama_client import OllamaClient
from skellysubs.ai_clients.openai_client import get_or_create_openai_client


def get_ai_client(ai_client_strategy: str="openai") -> AiClientABC:
    return AiClientStrategies.OPENAI.value if ai_client_strategy == "openai" else AiClientStrategies.OLLAMA.value

class AiClientStrategies(enum.Enum):
    OPENAI = get_or_create_openai_client()
    # OLLAMA = OllamaClient() #TODO - Add Ollama client
    # HUGGINGFACE = HuggingFaceClient() #TODO - Add HuggingFace client