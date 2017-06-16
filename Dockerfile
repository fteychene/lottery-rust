FROM debian:jessie-slim
LABEL maintainer "Francois Teychene <francois.teychene@gmail.com>"

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
    apt-get install \
       ca-certificates \
       curl \
       gcc \
       libc6-dev \
       -qqy \
       --no-install-recommends \
    && rm -rf /var/lib/apt/lists/*

ENV RUST_ARCHIVE=rust-nightly-x86_64-unknown-linux-gnu.tar.gz
ENV RUST_DOWNLOAD_URL=https://static.rust-lang.org/dist/$RUST_ARCHIVE

RUN mkdir -p /rust
WORKDIR /rust

RUN curl -fsOSL $RUST_DOWNLOAD_URL \
    && curl -s $RUST_DOWNLOAD_URL.sha256 | sha256sum -c - \
    && tar -C /rust -xzf $RUST_ARCHIVE --strip-components=1 \
    && rm $RUST_ARCHIVE \
    && ./install.sh

RUN apt-get update && \
    apt-get install -y \
       openssl \
       pkg-config \
       libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /rust/app
WORKDIR /rust/app

COPY . /rust/app
RUN cargo build --release

CMD ["cargo", "run", "--release"]