import logging
import multiprocessing
import time

import skellysubs
from skellysubs.api.server.server_singleton import create_server_manager
from skellysubs.app.skellysubs_app_state import create_skellysubs_app_state
from skellysubs.system.logging_configuration.configure_logging import configure_logging
from skellysubs.system.logging_configuration.log_test_messages import print_log_level_messages
from skellysubs.system.logging_configuration.logger_builder import LogLevels

logger = logging.getLogger(__name__)
configure_logging(LogLevels.TRACE)

if multiprocessing.current_process().name == "mainprocess":
    print_log_level_messages(logger)
logger.trace(f"Running {skellysubs.__package_name__} package, version: {skellysubs.__version__}, from file: {__file__}")

print("hiiiii")

def run_skellysubs_server(global_kill_flag: multiprocessing.Value):
    server_manager = create_server_manager(global_kill_flag=global_kill_flag)
    server_manager.start_server()
    while server_manager.is_running:
        time.sleep(1)
        if global_kill_flag.value:
            server_manager.shutdown_server()
            break

    logger.info("Server main process ended")


if __name__ == "__main__":

    multiprocessing.freeze_support()
    outer_global_kill_flag = multiprocessing.Value("b", False)
    try:
        create_skellysubs_app_state(global_kill_flag=outer_global_kill_flag)
        run_skellysubs_server(outer_global_kill_flag)
        outer_global_kill_flag.value = True
    except Exception as e:
        logger.error(f"Server main process ended with error: {e}")
        raise
    finally:
        outer_global_kill_flag.value = True
    print("Done!")
