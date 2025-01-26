from pathlib import Path


async def get_video_and_output_paths(video_name: str) -> tuple[str, str, str]:
    video_path = f'{video_name}.mp4' if not video_name.endswith('.mp4') else video_name
    subtitled_video_path = video_path.replace('.mp4', '_subtitled.mp4')
    translation_path = video_path.replace('.mp4', '_translation.json')
    if not Path(video_path).exists():
        raise FileNotFoundError(f"File not found: {video_path}")
    if not Path(video_path).is_file():
        raise ValueError(f"Path is not a file: {video_path}")
    return subtitled_video_path, video_path, translation_path


