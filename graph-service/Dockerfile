FROM ghcr.io/astral-sh/uv:0.11.31-python3.12-bookworm-slim AS build

ENV UV_COMPILE_BYTECODE=1 UV_LINK_MODE=copy
WORKDIR /app
COPY pyproject.toml uv.lock ./
RUN uv sync --frozen --no-dev --no-install-project
COPY room_graph ./room_graph
RUN uv sync --frozen --no-dev

FROM python:3.12.11-slim-bookworm
ENV PATH="/app/.venv/bin:$PATH" PYTHONUNBUFFERED=1
RUN useradd --create-home --uid 10001 graph
WORKDIR /app
COPY --from=build --chown=graph:graph /app /app
USER graph
EXPOSE 8000
HEALTHCHECK --interval=15s --timeout=5s --retries=5 \
  CMD ["python", "-c", "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8000/healthz', timeout=3)"]
CMD ["uvicorn", "room_graph.app:default_app", "--factory", "--host", "0.0.0.0", "--port", "8000", "--workers", "1"]
