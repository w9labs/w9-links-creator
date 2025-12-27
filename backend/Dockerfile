FROM rust:1.83-alpine as builder
WORKDIR /usr/src/app
RUN apk add --no-cache musl-dev
COPY . .
RUN cargo install --path .

FROM alpine:3.19
RUN apk add --no-cache libgcc
COPY --from=builder /usr/local/cargo/bin/w9-links-creator /usr/local/bin/w9-links-creator
CMD ["w9-links-creator"]
