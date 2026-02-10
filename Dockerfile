FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    build-essential \
    pkg-config \
    file \
    wget \
    libssl-dev \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libxdo-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    patchelf \
    python3 \
    python3-pip \
    python3-venv \
    python-is-python3 \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile default --default-toolchain stable \
    && rustc --version \
    && cargo --version

RUN npm install -g pnpm@10.29.2 \
    && pnpm --version

WORKDIR /workspace

CMD ["bash"]
