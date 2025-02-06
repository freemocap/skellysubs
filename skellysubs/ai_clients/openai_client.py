import json
import logging
from typing import Type

from openai import AsyncOpenAI, BaseModel
from openai.types.audio import TranscriptionVerbose

from skellysubs.ai_clients.ai_client_abc import AiClientConfigABC, AiClientABC, AiSystemMessage, AiUserMessage
from skellysubs.utilities.load_env_variables import OPENAI_API_KEY




logger = logging.getLogger(__name__)


class OpenAIClientConfig(AiClientConfigABC):
    model_name: str = "gpt-4o-mini"
    max_context_length:int = 128_000


class OpenaiClient(AiClientABC):
    config: OpenAIClientConfig = OpenAIClientConfig()
    client: AsyncOpenAI = AsyncOpenAI(api_key=OPENAI_API_KEY)



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

    async def make_whisper_transcription_request(self,
                                                 prompt: str,
                                                 audio_file_path: str) -> TranscriptionVerbose:
        audio_file = open(audio_file_path, "rb")
        transcript_response = await self.client.audio.transcriptions.create(
            file=audio_file,
            model="whisper-1",
            response_format="verbose_json",
            timestamp_granularities=["segment", "word"]
        )
        return transcript_response

_OPENAI_CLIENT: OpenaiClient | None = None


def get_or_create_openai_client() -> OpenaiClient:
    global _OPENAI_CLIENT
    if _OPENAI_CLIENT is None:
        _OPENAI_CLIENT = OpenaiClient()
    return _OPENAI_CLIENT
