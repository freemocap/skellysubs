"""The analysis backend for SkellySubs (github.com/freemocap/skellysubs)."""

__author__ = """Skelly FreeMoCap    """
__email__ = "info@freemocap.org"
__version__ = "v0.1.0"
__description__ = "Transcription, translation, and video subtitling (github.com/freemocap/skellysubs)."

__package_name__ = "skellysubs"
__repo_url__ = f"https://github.com/freemocap/{__package_name__}/"
__repo_issues_url__ = f"{__repo_url__}issues"

from pathlib import Path

SAMPLE_DATA_TRANSCRIPT_PATH = str(
    Path(__file__).parent.parent / "sample_data" / "sample_video_long" / "sample_video_long_transcription.json")
if not Path(SAMPLE_DATA_TRANSCRIPT_PATH).exists():
    raise FileNotFoundError(f"Sample data not found at {SAMPLE_DATA_TRANSCRIPT_PATH}")
from skellysubs.system.logging_configuration.configure_logging import configure_logging
from skellysubs.system.logging_configuration.logger_builder import LogLevels

# configure_logging(LogLevels.TRACE)
configure_logging(LogLevels.DEBUG)
