from datetime import datetime
from pathlib import Path
from time import time


def get_skellysubs_data_folder_path() -> str:
    path  =  Path().home() / "Sync" / "skellybot-data" / "skellysubs"
    path.mkdir(parents=True, exist_ok=True)
    return str(path)

def get_gmt_offset_string():
    # from - https://stackoverflow.com/a/53860920/14662833
    gmt_offset_int = int(time.localtime().tm_gmtoff / 60 / 60)
    return f"{gmt_offset_int:+}"


def get_iso6201_time_string(timespec: str = "milliseconds", make_filename_friendly: bool = True):
    iso6201_timestamp = datetime.now().isoformat(timespec=timespec)
    gmt_offset_string = f"_gmt{get_gmt_offset_string()}"
    iso6201_timestamp_w_gmt = iso6201_timestamp + gmt_offset_string
    if make_filename_friendly:
        iso6201_timestamp_w_gmt = iso6201_timestamp_w_gmt.replace(":", "_")
        iso6201_timestamp_w_gmt = iso6201_timestamp_w_gmt.replace(".", "ms")
    return iso6201_timestamp_w_gmt


def create_new_default_recording_name():
    full_time = get_iso6201_time_string(timespec="seconds")
    just_hours_minutes_seconds = full_time.split("T")[1]
    recording_name = "recording_" + just_hours_minutes_seconds
    return recording_name





def create_log_file_name():
    return "skellysubs_" + time.strftime("%Y-%m-%d_%H_%M_%S") + ".log"

def get_log_file_path():
    log_folder_path = Path(get_skellysubs_data_folder_path()) / 'logs'
    log_folder_path.mkdir(exist_ok=True, parents=True)
    log_file_path = log_folder_path / create_log_file_name()
    return str(log_file_path)