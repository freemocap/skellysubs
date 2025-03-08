
<p align="center">
    <img src="https://github.com/user-attachments/assets/3bb41434-fa1e-4603-a1d9-0072c090ba2a" height="240" alt="SkellySubs Project Logo">
</p> 

# SkellySubs 💀💬
Browser-based multilingual video translation and subtitling tool

## Overview
Skellysubs provides in-browser video translation and subtitling capabilities. Video processing is handled client-side using ffmpeg.wasm, keeping server load light enough to wrangle significant-ish traffic on a free-tier GCP Cloud Run instance 🤞😅

## Demo videos
![Demo (of the original python-only version)](https://github.com/user-attachments/assets/c7a3d9b4-f8d7-4728-a67e-fd7954c0e174)

![Roughly how it works](https://github.com/user-attachments/assets/089996cf-960d-4704-b5d9-f4ddf19d757b)




## Development Setup

### Backend
1. Install `uv`: https://docs.astral.sh/uv/getting-started/installation/
2. Create and activate virtual environment:
```bash
uv venv
.venv/Scripts/activate  # Windows
source .venv/bin/activate  # Unix
```
3. Install dependencies:
```bash
uv sync
```
4. Run the server:
```bash
python skellysubs/__main__.py
```
The API documentation will be available at `http://localhost:8080`

### Frontend
1. Navigate to the UI directory:
```bash
cd skellysubs-ui
```
2. Install dependencies:
```bash
npm install
```
3. Start development server:
```bash
npm run dev
```

## Deployment
Commits to the `production` branch automatically trigger deployment to Google Cloud Run.

## Data Privacy & Usage
- No video, audio, or translation data is stored on our servers  
- Translations are processed through OpenAI's API and subject to their standard privacy policy
- All video processing occurs in your browser (using a WebAssembly version of ffmpeg bundled along with the webpage) 
- OpenAI API costs are currently covered by the FreeMoCap Foundation as a service to the community
- If usage grows beyond our free-tier infrastructure capacity or token costs become prohibitive... we'll figure something out 😌❤️


## Docker
For local containerized testing:
```bash
docker build -t skellysubs . && docker run -p 8080:8080 --name skellysubs-docker skellysubs
```
