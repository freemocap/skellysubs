import logging
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI

import skellysubs
from skellysubs.skellysubs_app.api.server.server_constants import APP_URL
from skellysubs.system.files_and_folder_names import get_skellysubs_data_folder_path

logger = logging.getLogger(__name__)



@asynccontextmanager
async def lifespan(app: FastAPI):
    # Startup actions
    logger.api(f"SkellySubs API starting (app: {app})...")
    logger.info(f"SkellySubs API base folder path: {get_skellysubs_data_folder_path()}")
    Path(get_skellysubs_data_folder_path()).mkdir(parents=True, exist_ok=True)

    localhost_url = APP_URL.replace('0.0.0.0', 'localhost')
    logger.success(f"SkellySubs API (version:{skellysubs.__version__}) started successfully 💀🤖💬")
    logger.api(f"SkellySubs API  running on: \n\t\t\tSwagger API docs - {localhost_url} \n\t\t\tTest UI: {localhost_url}/ui 👈[click to open backend UI in your browser]")

    # Let the app do its thing
    yield

    # Shutdown actions
    logger.api("SkellySubs API ending...")
    logger.success("SkellySubs API shutdown complete - Goodbye!👋")
