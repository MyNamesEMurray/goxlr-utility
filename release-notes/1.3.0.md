## GoXLR Utility 1.3.0

### New: native desktop app

The browser-based interface has been replaced by `goxlr-ui`, a native desktop app (built with Slint, software
rendered, no web engine). It controls everything over the local IPC socket, so it needs no browser and no WebView2.

- Mixer with per-channel volume and mute, fader assignment and scribbles
- Routing matrix, sub mix and monitor mix
- Microphone: gate, compressor, EQ and a live level meter
- Effects, sampler, lighting, profiles and daemon settings
- Dark and light themes (remembered)
- **Import from the official GoXLR app**: Profiles -> Import scans the official app's folder and copies profiles
  and mic profiles into the utility

### Changed

- The web UI served by the daemon has been removed. The HTTP / websocket API is unchanged, so scripts and integrations
  (for example the StreamDeck plugin) keep working.
- "Open UI" starts `goxlr-ui` from next to the daemon. The separate Tauri wrapper app and WebView2 are no longer
  installed on Windows or macOS.
- Linux packages now include `goxlr-ui` (needs fontconfig).

### Notes

- Some slider ranges in the new UI are best guesses. If the daemon rejects a value the error shows in the header.
- Remote use from a browser on another computer is no longer available.
