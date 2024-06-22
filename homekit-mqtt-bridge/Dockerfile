FROM rust:1.72 as builder

COPY ./src ./homekit-mqtt-bridge/src
COPY ./Cargo.toml ./homekit-mqtt-bridge/Cargo.toml

WORKDIR ./homekit-mqtt-bridge

RUN apt-get update && apt-get install -y libssl-dev && apt-get install -y cmake

RUN cargo build --release

# Runtime stage
FROM debian:buster-slim

COPY --from=builder /homekit-mqtt-bridge/target/release/homekit-mqtt-bridge /usr/local/bin/homekit-mqtt-bridge

CMD ["/usr/local/bin/homekit-mqtt-bridge"]
