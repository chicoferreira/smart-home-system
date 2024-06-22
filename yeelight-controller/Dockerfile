FROM rust:1.72 as builder

COPY ./src ./yeelight-controller/src
COPY ./Cargo.toml ./yeelight-controller/Cargo.toml

WORKDIR ./yeelight-controller

RUN apt-get update && apt-get install -y libssl-dev && apt-get install -y cmake

RUN cargo build --release

FROM debian:buster-slim

COPY --from=builder /yeelight-controller/target/release/yeelight-controller /usr/local/bin/yeelight-controller

CMD ["/usr/local/bin/yeelight-controller"]
