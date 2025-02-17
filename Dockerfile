# Use a Python image with uv pre-installed
FROM ghcr.io/astral-sh/uv:python3.12-bookworm-slim as base

# Install system dependencies
RUN apt-get update && apt-get install -y \
    libgl1-mesa-glx \
    ffmpeg \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js and pnpm for building the UI
RUN curl -fsSL https://deb.nodesource.com/setup_23.x | bash - && \
    apt-get install -y nodejs && \
    npm install -g pnpm

# Set up a build stage for the UI
FROM base as build-ui

# Copy the app to the container
ADD . /app

# Set the working directory for building the UI
WORKDIR /app/skellysubs-ui

# Install dependencies and build the UI
RUN pnpm install && pnpm build

# Create the final stage to run the application
FROM base

# Copy built UI files to the final image
COPY --from=build-ui /app/skellysubs-ui/dist /app/skellysubs-ui/dist

# Set the working directory for the server
WORKDIR /app

# Sync dependencies for the Python app
RUN uv sync --frozen

ENV PYTHONPATH="/app"
ENV PORT=8080

CMD ["uv", "run", "skellysubs/run_skellysubs_server.py"]