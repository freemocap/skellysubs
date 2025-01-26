import logging

from fastapi import APIRouter, WebSocket

from skellysubs.api.websocket.websocket_server import SkellySubsWebsocketServer

logger = logging.getLogger(__name__)

skellysubs_websocket_router = APIRouter()


@skellysubs_websocket_router.websocket("/connect")
async def skellysubs_websocket_server_connect(websocket: WebSocket):
    """
    Websocket endpoint for client connection to the server - handles image data streaming to frontend.
    """

    await websocket.accept()
    logger.success(f"SkellySubs Websocket connection established!")

    async with SkellySubsWebsocketServer(websocket=websocket) as runner:
        await runner.run()
    logger.info("Websocket closed")
