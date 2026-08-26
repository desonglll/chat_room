FROM python:3.12.11-slim-bookworm AS build

COPY --from=ghcr.io/astral-sh/uv:0.11.31 /uv /uvx /bin/

ENV UV_COMPILE_BYTECODE=1 \
    UV_LINK_MODE=copy \
    UV_INDEX_URL=https://mirrors.tuna.tsinghua.edu.cn/pypi/web/simple
WORKDIR /app
COPY pyproject.toml uv.lock uv.toml ./
RUN uv sync --frozen --no-dev --no-install-project
COPY room_graph ./room_graph
RUN uv sync --frozen --no-dev && .venv/bin/python -c "import room_graph.app"

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
