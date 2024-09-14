# FROM --platform=linux/amd64 ubuntu:latest
FROM --platform=linux/aarch64 ubuntu:latest
RUN mkdir -p /usr/src/project
RUN apt update -y
RUN apt install clang -y
RUN apt install valgrind -y
RUN apt install curl -y
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
RUN $HOME/.cargo/bin/rustup default stable
RUN $HOME/.cargo/bin/rustup component add rust-analyzer
RUN $HOME/.cargo/bin/rustup target add wasm32-unknown-unknown
RUN $HOME/.cargo/bin/cargo install --locked trunk
# RUN cd /usr/src && curl -LO https://github.com/dotjulia/stackium/releases/latest/download/stackium && curl -LO https://github.com/dotjulia/stackium/releases/latest/download/build_test.sh
# CMD cd /usr/src/project && cp ../build_test.sh . && cp ../stackium . && chmod +x stackium