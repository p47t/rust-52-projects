# Project 52: Local Voice Assistant ("Jarvis")

A fully offline, local voice assistant that runs in the terminal. It combines real-time microphone capture, audio preprocessing via WebRTC, wakeword detection, voice activity detection (VAD), local multimodal LLM inference, and native text-to-speech feedback.

## Features

- **Background Wakeword Listening**: Continuously monitors audio from your microphone, filtering for the trigger word (e.g. "Alexa" or "Hey Mycroft") using `oww-rs` (openWakeWord).
- **WebRTC APM Preprocessing**: Applies pure-Rust noise suppression (NS) and Automatic Gain Control (AGC2) via `sonora` to clean the mic input before passing it to the detector or recorder.
- **Voice Activity Detection (VAD)**: When the wakeword is triggered, the assistant transitions to recording state, dynamically tracking audio RMS energy levels. It automatically stops recording after 1.5 seconds of silence or a maximum timeout (8 seconds) is reached.
- **Direct Multimodal Audio Inference**: Passes the captured voice command directly as an audio tensor embedding to the **Gemma-4 Multimodal Audio** model (`gemma-4-E4B-it`) using `llama-cpp-4` with GPU (Vulkan) acceleration. This bypasses separate speech-to-text (STT) and LLM stages for faster and more context-aware responses.
- **Voice Output (TTS)**: Synthesizes the text response using native Windows Speech Synthesis (`System.Speech`) for a zero-dependency, zero-compile-time local voice loop.

## Prerequisites

This project reuses the model and projector files downloaded for `llm-local-studio-4`. Before running the assistant, ensure you have:

1. **Gemma-4 GGUF Model**: `llm-local-studio-4/models/huggingface/lmstudio-community/gemma-4-E4B-it-GGUF/gemma-4-E4B-it-Q4_K_M.gguf`
2. **Gemma-4 Multimodal Projector**: `llm-local-studio-4/models/huggingface/lmstudio-community/gemma-4-E4B-it-GGUF/mmproj-gemma-4-E4B-it-BF16.gguf`

*If they are missing, run the download and serve scripts in `llm-local-studio-4` to fetch them.*

## How it Works

The system operates as a thread-safe async state machine:

1. **`State::Listening`**: Captures microphone input, resamples to 16kHz mono, and feeds 1280-sample blocks to openWakeWord.
2. **`State::Recording`**: Triggers upon wakeword detection. Saves speech samples and tracks silence frames.
3. **`State::Processing`**: Writes the recording to `temp_query.wav`, tokenizes it using the multimodal projector, and streams response tokens from Gemma-4 to the console.
4. **`State::Speaking`**: Sends the final text response to Windows PowerShell Speech Synthesizer and suspends microphone listening during playback to prevent self-triggering.

## Usage

Run the project using cargo:

```bash
# Run in release mode for fast ONNX and llama.cpp inference
cargo run --release
```

### CLI Options

```text
Usage: voice-assistant [OPTIONS]

Options:
  -m, --model <MODEL>
          Path to the GGUF model file 
          [default: ../llm-local-studio-4/models/huggingface/lmstudio-community/gemma-4-E4B-it-GGUF/gemma-4-E4B-it-Q4_K_M.gguf]
  -p, --mmproj <MMPROJ>
          Path to the mmproj GGUF file 
          [default: ../llm-local-studio-4/models/huggingface/lmstudio-community/gemma-4-E4B-it-GGUF/mmproj-gemma-4-E4B-it-BF16.gguf]
  -w, --wakeword <WAKEWORD>
          Wake word model to use [default: alexa] [possible values: alexa, mycroft]
  -t, --threshold <THRESHOLD>
          Confidence threshold for wakeword detection (0.0 to 1.0) [default: 0.5]
      --no-apm
          Disable WebRTC APM preprocessing
  -v, --vad-threshold <VAD_THRESHOLD>
          RMS energy silence threshold for VAD [default: 0.003]
  -d, --max-duration <MAX_DURATION>
          Maximum recording duration in seconds [default: 8.0]
  -s, --silence-duration <SILENCE_DURATION>
          Seconds of continuous silence before ending recording [default: 1.5]
  -h, --help
          Print help
```
