from abc import ABC, abstractmethod
from pydantic import BaseModel, Field, ConfigDict
from typing import Type

class AiMessagesABC(BaseModel, ABC):
    role: str
    content: str

class AiUserMessage(AiMessagesABC):
    role: str = Field(default="user", frozen=True)

class AiSystemMessage(AiMessagesABC):
    role: str = Field(default="system", frozen=True)

class AiClientConfigABC(BaseModel, ABC):
    temperature: float = Field(default=0.0, description="The temperature of the model. Higher values result in more random outputs.")
    model_name: str = Field(description="The name of the model to use.")

class AiClientABC(BaseModel, ABC):
    model_config = ConfigDict(arbitrary_types_allowed=True)

    @abstractmethod
    async def make_json_mode_request(self,
                                     system_prompt: str,
                                     prompt_model: Type[BaseModel],
                                     llm_model: str | None = None,
                                     temperature: float | None = None,
                                     user_input: str | None = None) -> BaseModel:
        pass