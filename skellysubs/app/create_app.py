import logging

from fastapi import FastAPI
from starlette.staticfiles import StaticFiles

from skellysubs.api.http.ui.ui_router import REACT_UI_ASSETS_FOLDER
from skellysubs.api.middleware.add_middleware import add_middleware
from skellysubs.api.middleware.cors import cors
from skellysubs.app.app_lifespan import lifespan
from skellysubs.app.app_setup import register_routes, customize_swagger_ui

logger = logging.getLogger(__name__)


def create_app() -> FastAPI:
    logger.api("Creating FastAPI app")
    app = FastAPI(lifespan=lifespan)
    app.mount("/assets", StaticFiles(directory=str(REACT_UI_ASSETS_FOLDER)), name="assets")
    cors(app)
    register_routes(app)
    add_middleware(app)
    customize_swagger_ui(app)
    return app
