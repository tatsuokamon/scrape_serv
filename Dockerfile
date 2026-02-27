# cpp build
	FROM ubuntu:22.04 AS cpp-builder
	RUN apt update && apt install -y build-essential cmake ninja-build git curl zip unzip pkg-config
	RUN git clone https://github.com/microsoft/vcpkg.git /vcpkg
	RUN /vcpkg/bootstrap-vcpkg.sh
	ENV VCPKG_ROOT=/vcpkg

	WORKDIR /cpp

	RUN mkdir ./build/
	COPY cpp/src/ ./src
	COPY cpp/include ./include
	COPY cpp/CMakeLists.txt .
	COPY cpp/vcpkg.json .

	RUN cmake -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_TOOLCHAIN_FILE=$VCPKG_ROOT/scripts/buildsystems/vcpkg.cmake -G Ninja
	RUN cmake --build build --config Release

# Rust build
	FROM rust:1.75-bookworm AS rust-builder
	WORKDIR /app

	COPY --from=cpp-builder /cpp/build/libcpp.a ./libcpp.a
	# need gumbo.a
	COPY --from=cpp-builder /cpp/build/vcpkg_installed/x64-linux/lib/libgumbo.a ./libgumbo.a

	COPY src ./src
	COPY Cargo.toml ./
	COPY build.rs ./

	RUN cargo build --release

# running stage
	FROM gcr.io/distroless/cc-debian12
	WORKDIR /app

	COPY --from=rust-builder /app/target/release/rust .
	COPY ./ .env .
	ENV PORT=8080
	# ENV REDIS_URL= // need 
	EXPOSE 8080

	CMD ["./rust"]
