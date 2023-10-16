FROM golang:1.21.3

WORKDIR /app

COPY go.mod go.sum ./
RUN go mod download

COPY *.go ./

RUN CGO_ENABLED=0 GOOS=linux go build -o /mqtt-topic-remapper

COPY config.toml ./config.toml

CMD ["/mqtt-topic-remapper"]