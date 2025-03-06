from huggingface_hub import InferenceClient

from skellysubs.utilities.load_env_variables import HUGGINGFACE_API_KEY

client = InferenceClient(
    provider="together",
    api_key=HUGGINGFACE_API_KEY
)

messages = [
    {
        "role": "user",
        "content": "Do only humans have a corticospinal tract?"
    }
]

completion = client.chat.completions.create(
    model="deepseek-ai/DeepSeek-R1",
    messages=messages,
)

print(completion.choices[0].message.content)
