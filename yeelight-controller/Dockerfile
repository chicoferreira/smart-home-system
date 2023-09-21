FROM rust:1.72

COPY ./src ./yeelight-controller/src
COPY ./Cargo.toml ./yeelight-controller/Cargo.toml

WORKDIR ./yeelight-controller

RUN apt-get update && apt-get install -y libssl-dev && apt-get install -y cmake

RUN cargo build --release

CMD ["./target/release/yeelight-controller"]
