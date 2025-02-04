# skellysubs
Multi-lingual translation and subtitling 

> WORK IN PROGRESS - Not yet open to external contributions (hopefully we can do that before the end of Feb '25)

https://github.com/user-attachments/assets/0bc27df0-9614-4716-8638-f0b130ef791d


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

(FYI - its all busted up atm - expect bugs, crashes, and loopdidoopty tech debty nonsense!) 
