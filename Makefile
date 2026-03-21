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
	open "$$(xcodebuild -project KoeApp/Koe.xcodeproj -scheme Koe -configuration Debug -showBuildSettings 2>/dev/null | grep ' BUILD_DIR' | head -1 | awk '{print $$3}')/Debug/Koe.app"
