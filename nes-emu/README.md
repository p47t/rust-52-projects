# nes-emu

NES emulator frontend built with the [Bevy](https://bevyengine.org/) game engine. Combines the emulation crates in this repository (`nes-cpu`, `nes-ppu`, `nes-apu`, `nes-joypad`, `nes-mapper`) into a playable emulator with CRT post-processing effects.

## Screenshot

![nes-emu screenshot](../docs/images/nes-emu.png)

## Features

- Full NES emulation (CPU, PPU, APU) via the companion crates
- CRT shader effects: barrel distortion, scanlines, RGB phosphor shadow mask, vignette
- Real-time audio output via cpal
- Keyboard input for player 1 joypad

## Usage

```bash
cd nes-emu
cargo run --release -- <rom-path>
```

## Controls

| Key | NES Button |
|-----|-----------|
| Arrow keys | D-pad |
| Z | A |
| X | B |
| A | Select |
| S | Start |

## Architecture

```
src/
  main.rs        App setup, window config, plugin registration
  emulation.rs   NES system (NonSend resource), per-frame emulation loop
  video.rs       Framebuffer texture, camera, display quad
  audio.rs       cpal output stream, shared ring buffer
  input.rs       Keyboard -> NES joypad mapping
  crt.rs         CRT material plugin, shader binding
assets/
  shaders/
    crt.wgsl     WGSL fragment shader for CRT effects
```

The NES `System` uses `Rc<RefCell<T>>` internally (non-Send), so it is stored as a Bevy `NonSend` resource and automatically scheduled on the main thread.

Audio uses cpal directly rather than Bevy's audio plugin, since real-time sample streaming from the APU requires a producer-consumer ring buffer rather than asset-based playback.

### CRT Shader

The CRT material is a custom `Material2d` with an embedded WGSL shader applied to a full-screen quad. Parameters are packed into a `vec4<f32>` uniform:

| Component | Parameter | Default |
|-----------|-----------|---------|
| r | Scanline intensity | 0.7 |
| g | Barrel curvature | 0.4 |
| b | Vignette intensity | 0.6 |
| a | Brightness boost | 1.3 |

## Feature Ideas

Potential enhancements enabled by the Bevy engine.

### Near-Zero Effort (built-in Bevy systems)

- **Fullscreen toggle** -- `window.mode = BorderlessFullscreen` on F11
- **Gamepad support** -- `Res<ButtonInput<GamepadButton>>` maps to NES joypad
- **FPS overlay** -- `FrameTimeDiagnosticsPlugin` + `Text2d` entity
- **Window resize handling** -- `WindowResized` event to rescale quad with correct aspect ratio
- **Pause/unpause** -- `State<AppState>` with `Paused`/`Running`, gate emulation on state

### Low Effort (< 50 lines each)

- **Screenshot** -- read `FramebufferHandle` image data, write PNG on keypress
- **Speed control** -- fast-forward by running N emulation steps per frame, slow-mo by skipping frames
- **Frame stepping** -- when paused, advance exactly one frame on keypress
- **CRT toggle** -- hotkey swaps between `CrtMaterial` and plain `ColorMaterial`
- **Volume control** -- scale samples in the ring buffer by a `Res<Volume>` float

### Medium Effort (bevy_egui overlay)

- **CRT tuning sliders** -- egui panel that writes to `CrtMaterial.params` live
- **Key remapping** -- egui panel + serialized config file
- **Channel mute toggles** -- checkboxes for pulse1/pulse2/triangle/noise/DMC
- **ROM picker** -- `rfd::FileDialog` on startup instead of CLI arg

### Why Bevy makes these easy

| Bevy feature | What it unlocks |
|---|---|
| ECS `Resource` | Any global state (volume, pause, speed) is trivially shared across systems |
| `State` machine | Pause/menu/gameplay transitions with automatic system scheduling |
| Built-in `Gamepad` input | No extra crate needed, same pattern as keyboard |
| `WindowResized` event | Reactive layout without polling |
| Asset hot-reloading | Edit `crt.wgsl` while running, see changes live |
| System ordering | `.before()` / `.after()` makes frame-step and speed control clean |
| `bevy_egui` ecosystem | Debug UI overlays without building a UI framework |

## Dependencies

- **bevy** 0.15 -- rendering, windowing, ECS
- **cpal** 0.15 -- cross-platform audio output
- **nes-cpu**, **nes-ppu**, **nes-apu**, **nes-joypad**, **nes-mapper** -- emulation core
