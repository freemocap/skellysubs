# Use a Python image with uv pre-installed
FROM ghcr.io/astral-sh/uv:python3.12-bookworm-slim
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /bin/
# Install system dependencies
RUN apt-get update && apt-get install -y \
    libgl1-mesa-glx \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*
ADD . /app

WORKDIR /app
RUN uv sync --frozen
ENV PYTHONPATH="/app"
ENV PORT=8080

CMD ["uv", "run", "skellysubs/run_skellysubs_server.py"]