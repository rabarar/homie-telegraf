FROM rust:1.73 as build

# create a new empty shell project
RUN USER=root cargo new --bin homie-input
WORKDIR /homie-input

# copy over your manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# this build step will cache your dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy your source tree
COPY ./src ./src

# build for release
RUN rm ./target/release/deps/homie_input*
RUN cargo build --release

# our final base
FROM rust:1.73

# copy the build artifact from the build stage
COPY --from=build /homie-input/target/release/homie-input .

# set the startup command to run your binary
ENTRYPOINT ["/homie-input", "-x", "influx", "-i",  "8086", "-g", "10402q-MD", "-o", "homie"]
