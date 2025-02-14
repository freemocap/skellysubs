import os

PROTOCOL = "http"
HOSTNAME = "0.0.0.0"
PORT = int(os.environ.get("PORT", 8080))  # Read PORT from environment variables
APP_URL = f"{PROTOCOL}://{HOSTNAME}:{PORT}"
