from skellysubs.api.http.app.health import health_router
from skellysubs.api.http.app.state import state_router
from skellysubs.api.http.core.transcribe.transcribe_router import transcribe_router
from skellysubs.api.http.ui.ui_router import ui_router
from skellysubs.api.websocket.websocket_connect import skellysubs_websocket_router

OTHER_ROUTERS = {}

SKELLYSUBS_ROUTERS = {
    "/ui": {
        "ui": ui_router
    },
    "/app": {
        "health": health_router,
        "state": state_router,
        # "shutdown": app_shutdown_router
    },
    "/websocket": {
        "connect/{session_id}": skellysubs_websocket_router
    },
    "/processing": {
        "transcribe": transcribe_router,
        # "translate": translate_router,
        # "match_words": match_words_router,

    },

    **OTHER_ROUTERS
}
