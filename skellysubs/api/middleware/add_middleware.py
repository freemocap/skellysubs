import logging

from fastapi import FastAPI

logger = logging.getLogger(__name__)


def add_middleware(app: FastAPI):
    logger.debug("Adding middleware...")

    @app.middleware("http")
    async def add_security_headers(request, call_next):
        response = await call_next(request)
        # Required for SharedArrayBuffer
        response.headers["Cross-Origin-Opener-Policy"] = "same-origin"
        response.headers["Cross-Origin-Embedder-Policy"] = "require-corp"
        return response
