from ollama import AsyncClient

OLLAMA_ASYNC_CLIENT = None

DEFAULT_OLLAMA_MODEL = 'llama3.2'
def get_ollama_client () -> AsyncClient:
    global OLLAMA_ASYNC_CLIENT
    if OLLAMA_ASYNC_CLIENT is None:
        OLLAMA_ASYNC_CLIENT = AsyncClient()
    return OLLAMA_ASYNC_CLIENT

if __name__ == "__main__":
    import asyncio
    ollama_client = get_ollama_client()


    async def chat():
        message = {'role': 'user', 'content': 'Whats decussation?'}
        response = await AsyncClient().chat(model=DEFAULT_OLLAMA_MODEL, messages=[message])
        print(response.message.content)


    asyncio.run(chat())