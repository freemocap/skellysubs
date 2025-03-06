import os

from dotenv import load_dotenv

load_dotenv()

OPENAI_API_KEY = os.getenv('OPENAI_API_KEY')
if not OPENAI_API_KEY:
    raise ValueError("OPENAI_API_KEY not found in environment variables!")

# HUGGINGFACE_API_KEY = os.getenv('HUGGINGFACE_API_KEY')
# if not HUGGINGFACE_API_KEY:
#     raise ValueError("Please set HUGGINGFACE_API_KEY in your .env file")
