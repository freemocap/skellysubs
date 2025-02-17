import logging
import os
from pathlib import Path

from fastapi import APIRouter
from starlette.responses import HTMLResponse, FileResponse
from starlette.staticfiles import StaticFiles

logger = logging.getLogger(__name__)

ui_router = APIRouter()

REACT_UI_DIST_FOLDER = Path(__file__).parent.parent.parent.parent.parent  / 'skellysubs-ui' / 'dist'
REACT_UI_ASSETS_FOLDER = REACT_UI_DIST_FOLDER / 'assets'
REACT_UI_INDEX_FILE = REACT_UI_DIST_FOLDER / 'index.html'

if not REACT_UI_DIST_FOLDER.exists():
    raise FileNotFoundError(f"React UI build folder not found at {REACT_UI_DIST_FOLDER}")
if not REACT_UI_ASSETS_FOLDER.exists():
    raise FileNotFoundError(f"React UI assets folder not found at {REACT_UI_ASSETS_FOLDER}")
if not REACT_UI_INDEX_FILE.exists():
    raise FileNotFoundError(f"React UI index.html file not found at {REACT_UI_INDEX_FILE}")


@ui_router.get("/", response_class=FileResponse)
def serve_react_ui():
    logger.info("Serving React UI to `/`")

    return FileResponse(str(REACT_UI_INDEX_FILE), status_code=200)


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(ui_router, host="localhost", port=8000)
