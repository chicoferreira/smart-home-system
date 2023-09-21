FROM rust:1.72

COPY ./src ./homekit-mqtt-bridge/src
COPY ./Cargo.toml ./homekit-mqtt-bridge/Cargo.toml

WORKDIR ./homekit-mqtt-bridge

RUN apt-get update && apt-get install -y libssl-dev && apt-get install -y cmake

RUN cargo build --release

CMD ["./target/release/homekit-mqtt-bridge"]
