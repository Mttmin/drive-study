# Natural-voice study audio. See tools/tts/README.md.
.PHONY: audio audio-setup audio-publish

audio:
	bash tools/tts/run.sh $(ARGS)

audio-publish:
	AUDIO_PUBLISH=1 bash tools/tts/run.sh

audio-setup:
	uv venv tools/tts/.venv --python python3
	uv pip install --python tools/tts/.venv/bin/python -r tools/tts/requirements.txt
	@echo "ffmpeg must be on PATH or at tools/tts/bin/ffmpeg (libmp3lame)."
