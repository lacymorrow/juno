cpal 0.15.3 from crates.io, with two changes in `src/host/coreaudio/macos/mod.rs` (search `JUNO PATCH`):

1. `build_input_stream_raw` returns a stopped stream; `play()` starts it. Upstream starts it while building, which lights the macOS microphone indicator.
2. `Device::audio_device_id()` is public.

Remove when Juno no longer needs to build the input ahead of the press, or upstream offers both.
