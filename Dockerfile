FROM alpine:3

ARG TARGETPLATFORM

COPY bin/ /tmp/bin/

RUN set -ex; \
    case "$TARGETPLATFORM" in \
      linux/amd64) cp /tmp/bin/appz-linux-x64-musl /usr/local/bin/appz ;; \
      linux/arm64) cp /tmp/bin/appz-linux-arm64-musl /usr/local/bin/appz ;; \
      *) echo "Unsupported platform: $TARGETPLATFORM" && exit 1 ;; \
    esac; \
    chmod +x /usr/local/bin/appz; \
    rm -rf /tmp/bin

WORKDIR /site

ENTRYPOINT ["appz"]
