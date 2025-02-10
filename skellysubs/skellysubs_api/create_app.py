import logging

from fastapi import FastAPI

from skellysubs.skellysubs_app.api.middleware.add_middleware import add_middleware
from skellysubs.skellysubs_app.api.middleware.cors import cors
from skellysubs.skellysubs_app.app_lifespan import lifespan
from skellysubs.skellysubs_app.app_setup import register_routes, customize_swagger_ui

logger = logging.getLogger(__name__)


def create_app() -> FastAPI:
    logger.api("Creating FastAPI app")
    app = FastAPI(lifespan=lifespan)
    cors(app)
    register_routes(app)
    add_middleware(app)
    customize_swagger_ui(app)
    return app
