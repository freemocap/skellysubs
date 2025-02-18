import logging
from pathlib import Path

from fastapi import FastAPI
from starlette.staticfiles import StaticFiles

from skellysubs.api.http.ui.ui_router import REACT_UI_ASSETS_FOLDER
from skellysubs.api.middleware.add_middleware import add_middleware
from skellysubs.api.middleware.cors import cors
from skellysubs.app.app_lifespan import lifespan
from skellysubs.app.app_setup import register_routes, customize_swagger_ui

logger = logging.getLogger(__name__)
SKELLYSUBS_UI_PUBLIC_FOLDER = Path(__file__).parent.parent.parent / "skellysubs-ui" / "public"
if not SKELLYSUBS_UI_PUBLIC_FOLDER.exists():
    raise FileNotFoundError(f"Could not find the public folder for the UI at {SKELLYSUBS_UI_PUBLIC_FOLDER}")


def create_app() -> FastAPI:
    logger.api("Creating FastAPI app")
    app = FastAPI(lifespan=lifespan)
    app.mount("/assets", StaticFiles(directory=str(REACT_UI_ASSETS_FOLDER)), name="assets")
    app.mount("/public", StaticFiles(directory=str(SKELLYSUBS_UI_PUBLIC_FOLDER)), name="public")
    cors(app)
    register_routes(app)
    add_middleware(app)
    customize_swagger_ui(app)
    return app
