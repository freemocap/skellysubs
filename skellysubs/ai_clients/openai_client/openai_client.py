from openai import AsyncOpenAI

from skellysubs.utilities.load_env_variables import OPENAI_API_KEY

OPENAI_CLIENT = AsyncOpenAI(api_key=OPENAI_API_KEY)
DEFAULT_LLM = "gpt-4o-mini"
MAX_TOKEN_LENGTH = 128_000
