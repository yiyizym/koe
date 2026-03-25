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
		/workspace/FunASR/runtime/websocket/build/bin/funasr-wss-server-2pass \
		--download-model-dir /workspace/models \
		--model-dir damo/speech_paraformer-large-vad-punc_asr_nat-zh-cn-16k-common-vocab8404-onnx \
		--online-model-dir damo/speech_paraformer-large_asr_nat-zh-cn-16k-common-vocab8404-online-onnx \
		--vad-dir damo/speech_fsmn_vad_zh-cn-16k-common-onnx \
		--punc-dir damo/punc_ct-transformer_zh-cn-common-vad_realtime-vocab272727-onnx \
		--itn-dir thuduj12/fst_itn_zh \
		--lm-dir damo/speech_ngram_lm_zh-cn-ai-wesp-fst \
		--port 10095 \
		--certfile "" \
		--keyfile "" \
		--hotword /workspace/FunASR/runtime/websocket/hotwords.txt
