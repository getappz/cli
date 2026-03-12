# Build Go html-to-markdown as C shared library
FROM golang:1.23-bookworm AS go-build
WORKDIR /go-html-to-md
COPY go-html-to-md/ .
RUN go mod tidy && go build -buildmode=c-shared -o libhtml2md.so . && \
    install -Dvm755 libhtml2md.so /out/lib/libhtml2md.so && \
    install -Dvm644 libhtml2md.h /out/include/libhtml2md.h

# Build Rust http-server (workspace)
FROM rust:slim AS build
ENV DEBIAN_FRONTEND=noninteractive
WORKDIR /usr/src/appzcrawl

RUN apt-get update && apt-get install -yf make pkg-config gcc upx-ucl libssl-dev

# Copy Go artifacts
COPY --from=go-build /out/lib/libhtml2md.so /usr/local/lib/
COPY --from=go-build /out/include/libhtml2md.h /usr/local/include/
RUN ldconfig

# Copy workspace root and all crates
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Tell build.rs where to find libhtml2md (rust-lld doesn't search /usr/local/lib by default)
ENV HTML2MD_LIB_PATH=/usr/local/lib

RUN cargo build -p appzcrawl-server --release && \
    install -Dsvm755 target/release/appzcrawl-server ./bin/appzcrawl-server && \
    upx --best --lzma ./bin/appzcrawl-server

FROM gcr.io/distroless/cc:latest AS final
WORKDIR /app
COPY --from=build /usr/src/appzcrawl/bin /app
COPY --from=build /usr/local/lib/libhtml2md.so /app/lib/
COPY --from=build /usr/lib/x86_64-linux-gnu/libssl.so* /app/lib/
COPY --from=build /usr/lib/x86_64-linux-gnu/libcrypto.so* /app/lib/

ENV LD_LIBRARY_PATH=/app/lib
ENV PORT=4000
EXPOSE 4000
ENTRYPOINT ["/app/appzcrawl-server"]
