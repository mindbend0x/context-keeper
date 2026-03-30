# Devbox MCP Server — lightweight Python container
FROM python:3.12-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY server.py .

ENV DEVBOX_MCP_PORT=4000

EXPOSE 4000

CMD ["python", "server.py"]
