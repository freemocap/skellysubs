import os
from pathlib import Path

from dotenv import load_dotenv

load_dotenv()

OPENAI_API_KEY = os.getenv('OPENAI_API_KEY')


# Ensure the environment variables are set
if not OPENAI_API_KEY:
    raise ValueError("Please set OPENAI_API_KEY in your .env file")
