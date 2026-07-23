FROM debian:13-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home appz
USER appz
WORKDIR /home/appz

RUN curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/getappz/cli/main/install.sh | sh

ENV PATH="/home/appz/.local/bin:${PATH}"

ENTRYPOINT ["appz"]
