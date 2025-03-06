import logging

from .log_test_messages import log_test_messages
from .logger_builder import LoggerBuilder, LogLevels

# Suppress some external loggers that are too verbose for our context/taste
logging.getLogger("tzlocal").setLevel(logging.WARNING)
logging.getLogger("matplotlib").setLevel(logging.WARNING)
logging.getLogger("httpx").setLevel(logging.WARNING)
logging.getLogger("asyncio").setLevel(logging.WARNING)
logging.getLogger("websockets").setLevel(logging.INFO)
logging.getLogger("websocket").setLevel(logging.INFO)
logging.getLogger("watchfiles").setLevel(logging.WARNING)
logging.getLogger("httpcore").setLevel(logging.WARNING)
logging.getLogger("urllib3").setLevel(logging.WARNING)
logging.getLogger("openai").setLevel(logging.WARNING)
logging.getLogger("numba").setLevel(logging.WARNING)
logging.getLogger("jieba").setLevel(logging.WARNING)
logging.getLogger("python_multipart").setLevel(logging.WARNING)

# Add custom log levels
logging.addLevelName(LogLevels.GUI.value, "GUI")
logging.addLevelName(LogLevels.LOOP.value, "LOOP")
logging.addLevelName(LogLevels.TRACE.value, "TRACE")
logging.addLevelName(LogLevels.SUCCESS.value, "SUCCESS")
logging.addLevelName(LogLevels.API.value, "API")


def add_log_method(level: LogLevels, name: str):
    def log_method(self, message, *args, **kws):
        if self.isEnabledFor(level.value):
            self._log(level.value, message, args, **kws, stacklevel=2)

    setattr(logging.Logger, name, log_method)


def configure_logging(level: LogLevels = LogLevels.DEBUG):
    add_log_method(LogLevels.GUI, 'gui')
    add_log_method(LogLevels.LOOP, 'loop')
    add_log_method(LogLevels.TRACE, 'trace')
    add_log_method(LogLevels.API, 'api')
    add_log_method(LogLevels.SUCCESS, 'success')

    builder = LoggerBuilder(level)
    builder.configure()


if __name__ == "__main__":
    logger_test = logging.getLogger(__name__)
    log_test_messages(logger_test)
    logger_test.success(
        "Logging setup and tests completed. Check the console output and the log file."
    )
