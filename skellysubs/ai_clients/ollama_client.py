import asyncio
import json
from typing import Type

from ollama import AsyncClient
from pydantic import BaseModel, Field

from skellysubs.ai_clients.ai_client_abc import AiClientConfigABC, AiClientABC, AiSystemMessage, AiUserMessage

# Initialize the Ollama async client
OLLAMA_ASYNC_CLIENT = None

def get_or_create_ollama_client() -> AsyncClient:
    global OLLAMA_ASYNC_CLIENT
    if OLLAMA_ASYNC_CLIENT is None:
        OLLAMA_ASYNC_CLIENT = AsyncClient()
    return OLLAMA_ASYNC_CLIENT


class OllamaClientConfig(AiClientConfigABC):
    model_name: str = Field(default="phi4")


class OllamaClient(AiClientABC):
    config: OllamaClientConfig = OllamaClientConfig()
    client: AsyncClient = get_or_create_ollama_client()

    async def make_json_mode_request(self,
                                     system_prompt: str,
                                     prompt_model: Type[BaseModel],
                                     llm_model: str | None = None,
                                     temperature: float | None = None,
                                     user_input: str | None = None) -> BaseModel:
        messages = [
            AiSystemMessage(role="system", content=system_prompt).model_dump()
        ]
        if user_input is not None:
            messages.append(
                AiUserMessage(role="user", content=user_input).model_dump()
            )

        # Make the request to the Ollama client
        response = await self.client.chat(
            model=self.config.model_name if llm_model is None else llm_model,
            messages=messages,
            format=prompt_model.model_json_schema(),
            options={"temperature": self.config.temperature if temperature is None else temperature},
        )

        # Parse the response into the Pydantic model
        output = prompt_model(**json.loads(response.message.content))
        return output


# Run the example
if __name__ == "__main__":  # Define your Pydantic model
    class Country(BaseModel):
        name: str
        capital: str
        languages: list[str]
        history: str
        culture: str
        population: int
        best_attractions: list[str]
        recommendations: list[str]


    client = OllamaClient()


    async def run_client():
        response = await client.make_json_mode_request(system_prompt="Tell me about Syria.",
                                                       prompt_model=Country,
                                                       )
        print(response)


    asyncio.run(run_client())
