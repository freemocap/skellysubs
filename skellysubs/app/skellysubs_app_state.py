import logging
import multiprocessing
from dataclasses import dataclass

logger = logging.getLogger(__name__)


@dataclass
class SkellySubsAppState:
    global_kill_flag: multiprocessing.Value
    websocket_queue: multiprocessing.Queue

    @classmethod
    def create(cls, global_kill_flag: multiprocessing.Value):
        return cls(global_kill_flag=global_kill_flag,
                   websocket_queue=multiprocessing.Queue()
                   )

    def close(self):
        self.global_kill_flag.value = True


SKELLYSUBS_APP_STATE: SkellySubsAppState | None = None


def create_skellysubs_app_state(global_kill_flag: multiprocessing.Value) -> SkellySubsAppState:
    global SKELLYSUBS_APP_STATE
    if SKELLYSUBS_APP_STATE is None:
        SKELLYSUBS_APP_STATE = SkellySubsAppState.create(global_kill_flag=global_kill_flag)
    else:
        raise ValueError("SkellyBotAnalysis already exists!")
    return SKELLYSUBS_APP_STATE


def get_skellysubs_app_state() -> SkellySubsAppState:
    global SKELLYSUBS_APP_STATE
    if SKELLYSUBS_APP_STATE is None:
        raise ValueError("SkellyBotAnalysis does not exist!")
    return SKELLYSUBS_APP_STATE
