import logging

from fastapi import APIRouter

HELLO_FROM_SKELLYSUBS_BACKEND_MESSAGE = {"message": "Hello from the SkellySubs Backend 💀🤖✨"}

logger = logging.getLogger(__name__)
health_router = APIRouter()


@health_router.get("/health", summary="Hello👋")
def healthcheck_endpoint():
    """
    A simple endpoint to greet the user of the SkellySubs API.

    This can be used as a sanity check to ensure the API is responding.
    """

    logger.api("Hello requested! Deploying Hello!")
    return HELLO_FROM_SKELLYSUBS_BACKEND_MESSAGE
