from pathlib import Path


async def get_video_and_output_paths(video_path: str) -> tuple[str, str, str]:
    extension = Path(video_path).suffix
    subtitled_video_path = video_path.replace(f'{extension}', '_subtitled.mp4')
    translation_path = video_path.replace(f'{extension}', '_translation.json')
    if not Path(video_path).exists():
        raise FileNotFoundError(f"File not found: {video_path}")
    if not Path(video_path).is_file():
        raise ValueError(f"Path is not a file: {video_path}")
    return subtitled_video_path, video_path, translation_path
