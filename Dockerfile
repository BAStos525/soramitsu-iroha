#base stage
FROM archlinux:base-devel AS builder

ENV NIGHTLY=nightly-2022-08-15
COPY ./rust-toolchain.toml .
RUN set -eux && \
    pacman -Syu rustup mold musl rust-musl aarch64-linux-gnu-gcc --noconfirm && \
    # toolchain: ./rust-toolchain.toml
    rustup target add aarch64-unknown-linux-musl  && \
    rustup component add rust-src llvm-tools-preview  && \
    # toolchain: $NIGHTLY
    rustup install --profile default $NIGHTLY && \
    rustup +$NIGHTLY target add aarch64-unknown-linux-musl wasm32-unknown-unknown && \
    rustup +$NIGHTLY component add rust-src llvm-tools-preview && \
    # cargo install
    cargo install cargo-lints webassembly-test-runner && \
    :

# builder stage
WORKDIR /iroha
COPY . .
RUN  rm -f rust-toolchain.toml
RUN  cargo build --profile deploy --target aarch64-unknown-linux-musl --features vendored

# final image
FROM --platform=linux/arm64/v8 alpine:3.16

ARG  STORAGE=/storage
ARG  TARGET_DIR=/iroha/target/aarch64-unknown-linux-musl/deploy
ENV  BIN_PATH=/usr/local/bin/
ENV  CONFIG_DIR=/config
ENV  IROHA2_CONFIG_PATH=$CONFIG_DIR/config.json
ENV  IROHA2_GENESIS_PATH=$CONFIG_DIR/genesis.json
ENV  KURA_BLOCK_STORE_PATH=$STORAGE

RUN  set -ex && \
     apk --update add curl ca-certificates && \
     adduser --disabled-password iroha --shell /bin/bash --home /app && \
     mkdir -p $CONFIG_DIR && \
     mkdir $STORAGE && \
     chown iroha:iroha $STORAGE

COPY --from=builder $TARGET_DIR/iroha $BIN_PATH
COPY --from=builder $TARGET_DIR/iroha_client_cli $BIN_PATH
COPY --from=builder $TARGET_DIR/kagami $BIN_PATH
USER iroha
CMD  iroha
