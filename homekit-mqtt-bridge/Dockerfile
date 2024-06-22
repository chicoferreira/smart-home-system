FROM rust:1.72 as builder

COPY ./src ./homekit-mqtt-bridge/src
COPY ./Cargo.toml ./homekit-mqtt-bridge/Cargo.toml

WORKDIR ./homekit-mqtt-bridge

RUN apt-get update && apt-get install -y cmake

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y openssl

COPY --from=builder /homekit-mqtt-bridge/target/release/homekit-mqtt-bridge /usr/local/bin/homekit-mqtt-bridge

CMD ["/usr/local/bin/homekit-mqtt-bridge"]
