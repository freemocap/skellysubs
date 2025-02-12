from skellysubs.app.api.http.app.health import health_router
from skellysubs.app.api.http.app.shutdown import app_shutdown_router
from skellysubs.app.api.http.app.state import state_router
from skellysubs.app.api.http.subtitle_endpoint import subtitle_router
from skellysubs.app.api.http.ui.ui_router import ui_router
from skellysubs.app.api.websocket.websocket_connect import skellysubs_websocket_router

OTHER_ROUTERS = {}

SKELLYSUBS_ROUTERS = {
    "/ui": {
        "ui": ui_router
    },
    "/app": {
        "health": health_router,
        "state": state_router,
        "shutdown": app_shutdown_router
    },
    "/websocket": {
        "/connect/{session_id}": skellysubs_websocket_router
    },
    "/subtitles": {
        "subtitle_video": subtitle_router
    },

    **OTHER_ROUTERS
}
