# Use a Python image with uv pre-installed
FROM ghcr.io/astral-sh/uv:python3.12-bookworm-slim
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /bin/

# Copy the project into the image
ADD . /app

# Sync the project into a new environment, using the frozen lockfile
WORKDIR /app
RUN uv sync --frozen
ENV PYTHONPATH="${PYTHONPATH}:/app"

# Ensure the application listens on the port defined by the PORT environment variable
ENV PORT 8101

# Update the command to ensure it listens on the PORT environment variable
CMD ["uv", "run", "--port", "8080", "skellysubs/run_skellysubs_server.py"]