FROM rust:latest as builder
WORKDIR /usr/src/consumer
COPY . .
RUN unzip protoc-24.2-linux-x86_64.zip -d /usr/local/ && cargo install --path .


FROM debian:12-slim
ENV LANG C.UTF-8
RUN sed -i 's|http://deb.debian.org|http://mirrors.ustc.edu.cn|g' /etc/apt/sources.list.d/debian.sources \
    && apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/bin/protoc /usr/local/bin/
COPY --from=builder /usr/local/cargo/bin/consumer /usr/local/bin/
ENTRYPOINT ["consumer"]