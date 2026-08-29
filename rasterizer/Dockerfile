# Build from the forme/ repo root:
#   docker build -f rasterizer/Dockerfile -t formepdf/rasterizer .
#
# Supports multi-arch: builds for linux/amd64 and linux/arm64.

FROM rust:1.98-trixie AS builder

WORKDIR /app

# Copy manifests for dependency layer caching
COPY rasterizer/Cargo.toml rasterizer/Cargo.lock* rasterizer/

# Create dummy main.rs to pre-build dependencies
RUN mkdir -p rasterizer/src && echo "fn main() {}" > rasterizer/src/main.rs
WORKDIR /app/rasterizer
RUN cargo build --release 2>/dev/null || true

# Copy actual source
COPY rasterizer/src/ src/

# Touch main.rs so cargo rebuilds it (not just the cached dummy)
RUN touch src/main.rs
RUN cargo build --release

# --- Runtime stage ---
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

# Download PDFium binary for the target architecture
ARG TARGETARCH
ARG PDFIUM_VERSION=chromium/8021
RUN set -eux; \
    case "$TARGETARCH" in \
      amd64) PDFIUM_ARCH="linux-x64" ;; \
      arm64) PDFIUM_ARCH="linux-arm64" ;; \
      *) echo "Unsupported arch: $TARGETARCH" && exit 1 ;; \
    esac; \
    curl -sL "https://github.com/bblanchon/pdfium-binaries/releases/download/${PDFIUM_VERSION}/pdfium-${PDFIUM_ARCH}.tgz" \
      | tar xzf - -C /usr/local lib/libpdfium.so; \
    ldconfig

COPY --from=builder /app/rasterizer/target/release/forme-rasterizer /usr/local/bin/forme-rasterizer

RUN useradd -r -s /bin/false forme
USER forme

ENV RASTERIZER_PORT=3001
EXPOSE 3001

CMD ["forme-rasterizer"]
