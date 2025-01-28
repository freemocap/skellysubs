import json
import logging
from typing import Type

from openai import AsyncOpenAI, BaseModel

from skellysubs.ai_clients.ai_client_abc import AiClientConfigABC, AiClientABC, AiSystemMessage, AiUserMessage
from skellysubs.utilities.load_env_variables import OPENAI_API_KEY

_OPENAI_CLIENT: AsyncOpenAI | None = None


def get_or_create_openai_client() -> AsyncOpenAI:
    global _OPENAI_CLIENT
    if _OPENAI_CLIENT is None:
        _OPENAI_CLIENT = AsyncOpenAI(api_key=OPENAI_API_KEY)
    return _OPENAI_CLIENT



logger = logging.getLogger(__name__)


class OpenAIClientConfig(AiClientConfigABC):
    model_name: str = "gpt-4o-mini"
    max_context_length:int = 128_000


class OpenaiClient(AiClientABC):
    config: OpenAIClientConfig = OpenAIClientConfig()
    client: AsyncOpenAI = get_or_create_openai_client()



    async def make_json_mode_request(self,
                                     system_prompt: str,
                                     prompt_model: Type[BaseModel],
                                     llm_model: str | None = None,
                                     temperature: float | None = None,
                                     user_input: str | None = None) -> BaseModel:
        logger.trace(
            f"make_openai_json_mode_ai_request: prompt_model={prompt_model.__class__.__name__}, llm_model={llm_model}")
        messages = [
            AiSystemMessage(role="system", content=system_prompt).model_dump()
        ]
        if user_input is not None:
            messages.append(
                AiUserMessage(role="user", content=user_input).model_dump()
            )
        response = await self.client.beta.chat.completions.parse(
            model=self.config.model_name if llm_model is None else llm_model,
            messages=messages,
            response_format=prompt_model,
            temperature=self.config.temperature if temperature is None else temperature,
        )
        output = prompt_model(**json.loads(response.choices[0].message.content))

        logger.trace(f"make_openai_json_mode_ai_request: output={output.__class__.__name__}")
        return output

    async def make_text_generation_request(self,
                                           system_prompt: str,
                                           llm_model: str | None = None,
                                           temperature: float | None = None,
                                           user_input: str | None = None) -> str:
        messages = [
            AiSystemMessage(role="system", content=system_prompt).model_dump()
        ]
        response = await self.client.beta.chat.completions.parse(
            model=self.config.default_llm if llm_model is None else llm_model,
            messages=messages,
            temperature=self.config.temperature if temperature is None else temperature,
        )
        output = response.choices[0].message.content
        return output
