import asyncio
import logging
import multiprocessing
import uuid
from typing import Optional

from pydantic import BaseModel
from starlette.websockets import WebSocket, WebSocketState, WebSocketDisconnect

from skellysubs.app.skellysubs_app_state import get_skellysubs_app_state, \
    SkellySubsAppState

logger = logging.getLogger(__name__)

class WebsocketPayload(BaseModel):
    payload: str|dict|list|BaseModel
    session_id: uuid.UUID

class SkellySubsWebsocketServer:
    def __init__(self, websocket: WebSocket, session_id: str):
        self.websocket = websocket
        self.session_id = session_id
        self._skellysubs_app_state: SkellySubsAppState = get_skellysubs_app_state()
        self.frontend_image_relay_task: Optional[asyncio.Task] = None

    async def __aenter__(self):
        logger.debug("Entering SkellySubs  WebsocketServer context manager...")
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        logger.debug(" SkellySubs  WebsocketServer context manager exiting...")
        if not self.websocket.client_state == WebSocketState.DISCONNECTED:
            await self.websocket.close()

    async def run(self):
        logger.info("Starting websocket runner...")
        try:
            await asyncio.gather(
                asyncio.create_task(self._websocket_queue_relay()),
            )
        except Exception as e:
            logger.exception(f"Error in websocket runner: {e.__class__}: {e}")
            raise

    async def _websocket_queue_relay(self):
        """
        Relay messages from the sub-processes to the frontend via the websocket.
        """
        logger.info("Websocket relay listener started...")

        try:
            while True:
                if self._skellysubs_app_state.websocket_queue.qsize() > 0:
                    try:
                        message = self._skellysubs_app_state.websocket_queue.get()
                        logger.info(f"Got message from queue: {message}")
                        if not isinstance(message, WebsocketPayload):
                            raise ValueError(f"Message is not of type WebsocketPayload: {message}")
                        if message.session_id != self.session_id:
                            raise ValueError(f"Session ID does not match: {message.session_id} != {self.session_id}")
                        await self.websocket.send_json(message.model_dump())

                        # await self._handle_ipc_queue_message(
                        #     message=self._skellysubs_app_state.websocket_queue.get())
                    except multiprocessing.queues.Empty:
                        continue
                else:
                    await asyncio.sleep(1)

        except WebSocketDisconnect:
            logger.api("Client disconnected, ending listener task...")
        except asyncio.CancelledError:
            pass
        finally:
            logger.info("Ending listener for frontend payload messages in queue...")
        logger.info("Ending listener for client messages...")




