# skellysubs
Multi-lingual translation and subtitling 

> WORK IN PROGRESS - Not yet open to external contributions (hopefully we can do that before the end of Feb '25)
>
> It runs from the `skellysubs/__main__.py` file but expect bugs, crashes, and loopdidoopty tech debty nonsense!



https://github.com/user-attachments/assets/c7a3d9b4-f8d7-4728-a67e-fd7954c0e174


Original demo video - [https://github.com/user-attachments/assets/0bc27df0-9614-4716-8638-f0b130ef791d](https://github.com/user-attachments/assets/0bc27df0-9614-4716-8638-f0b130ef791d)


## Installation
- Install `uv` - https://docs.astral.sh/uv/getting-started/installation/
- create virtual environment -  `uv venv`
- activate virtual environment - `.venv/Scripts/activate`
- install dependencies - `uv sync`
- install torch with: - `pip3 install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu124`
- install whisper with: - `uv pip install git+https://github.com/openai/whisper.git `


## Usage
- Change video path in `__main__.py`
- run with `python skellysubs/__main__.py`


## Docker 

```
docker build -t skellysubs . && docker run -p 8101:8101 --name skellysubs-docker skellysubs
```

