# https://platform.openai.com/docs/guides/text-generation?text-generation-quickstart-example=json
import asyncio
import json
from typing import Type

from openai import AsyncOpenAI
from pydantic import BaseModel



async def make_openai_json_mode_ai_request(client: AsyncOpenAI,
                                           system_prompt: str,
                                           prompt_model: Type[BaseModel],
                                           llm_model: str,
                                           user_input: str | None = None,
                                           results_list: list | None = None):
    messages = [
        {
            "role": "system",
            "content": system_prompt
        }
    ]
    if user_input is not None:
        messages.append(
            {
                "role": "user",
                "content": user_input
            }
        )
    response = await client.beta.chat.completions.parse(
        model=llm_model,
        messages=messages,
        response_format=prompt_model
    )
    output = prompt_model(**json.loads(response.choices[0].message.content))
    if results_list is not None:
        results_list.append(output)
    return output


# if __name__ == "__main__":
    # from skellysubs.ai_clients.openai_client.openai_client import OPENAI_CLIENT, DEFAULT_LLM
    #
    # system_prompt = "Analyze the following text\nProduce your response using the following JSON schema instructions:\n\n" + TextAnalysisPromptModel.as_description_schema()
    # outer_user_input = "The quick brown fox jumps over the lazy dog. Foxes are mammals. The mammallian nervous system is includes the basal ganglia, the cerebellum, and the cerebral cortex, among other structures. Fox fur is often red or orange. Foxes are omnivores. "
    #
    # results = []
    # response = asyncio.run(make_openai_json_mode_ai_request(client=OPENAI_CLIENT,
    #                                                         system_prompt=system_prompt,
    #                                                         user_input=outer_user_input,
    #                                                         prompt_model=TextAnalysisPromptModel,
    #                                                         llm_model=DEFAULT_LLM,
    #                                                         results_list=results
    #                                                         ))
    # print(response)
    # print(f"len(results)={len(results)}")
