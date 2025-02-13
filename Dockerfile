# Use a Python image with uv pre-installed
FROM ghcr.io/astral-sh/uv:python3.12-bookworm-slim
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /bin/

ADD . /app

WORKDIR /app
RUN uv sync --frozen
ENV PYTHONPATH="${PYTHONPATH}:/app"

ENV PORT 8101

CMD ["uv", "run", "skellysubs/run_skellysubs_server.py"]