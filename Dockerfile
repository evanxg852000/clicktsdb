FROM rust:alpine3.17 as builder

WORKDIR /clicktsdb

RUN apk update && apk add --no-cache make protobuf-dev musl-dev

COPY . .

RUN cargo build --release

FROM alpine:3.17

COPY --from=builder /clicktsdb/target/release/clicktsdb /usr/local/bin/clicktsdb
COPY --from=builder /clicktsdb/configs/clicktsdb.yml /usr/local/bin/configs/clicktsdb.yml

RUN clicktsdb --version

ENTRYPOINT ["clicktsdb"]

