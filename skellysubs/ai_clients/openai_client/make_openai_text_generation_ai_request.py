# https://platform.openai.com/docs/guides/text-generation?text-generation-quickstart-example=json
import asyncio

from openai import AsyncOpenAI


async def make_openai_text_generation_ai_request(client: AsyncOpenAI,
                                           system_prompt: str,
                                           llm_model: str):
    messages = [
        {
            "role": "system",
            "content": system_prompt
        }
    ]
    response = await client.beta.chat.completions.parse(
        model=llm_model,
        messages=messages,
        temperature=0.0,
    )
    output = response.choices[0].message.content
    return output


if __name__ == "__main__":
    from skellybot_analysis.ai.clients.openai_client.openai_client import OPENAI_CLIENT, DEFAULT_LLM

    system_prompt = "Translate this statement into spanish, french, arabic (both in arabic script and ALA-LC romanization), and chinese (both simplified mandarin characters and pinyin romanization): 'The quick brown fox jumps over the lazy dog.'"

    results = []
    response = asyncio.run(make_openai_text_generation_ai_request(client=OPENAI_CLIENT,
                                                            system_prompt=system_prompt,
                                                            llm_model=DEFAULT_LLM,
                                                            ))
    print(response)
    print(f"len(results)={len(results)}")
