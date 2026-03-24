.PHONY: build build-rust build-xcode clean run

ARCH := aarch64-apple-darwin

build: build-rust build-xcode

build-rust:
	cargo build --manifest-path koe-core/Cargo.toml --release --target $(ARCH)

build-xcode:
	cd KoeApp && xcodebuild -project Koe.xcodeproj -scheme Koe -configuration Release build

clean:
	cargo clean
	cd KoeApp && xcodebuild -project Koe.xcodeproj -scheme Koe clean

run:
	open "$$(xcodebuild -project KoeApp/Koe.xcodeproj -scheme Koe -configuration Release -showBuildSettings 2>/dev/null | grep ' BUILD_DIR' | head -1 | awk '{print $$3}')/Release/Koe.app"

funasr:
	docker run -p 10096:10095 -it --privileged=true \
		-v ~/funasr-models:/workspace/models \
		registry.cn-hangzhou.aliyuncs.com/funasr_repo/funasr:funasr-runtime-sdk-online-cpu-0.1.13 \
		bash -c "cd /workspace/FunASR/runtime && bash run_server_2pass.sh --download-model-dir /workspace/models --certfile 0"
