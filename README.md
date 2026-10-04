# notslack

**Slack, without the browser.** 1.16 GB of RAM down to 165 MB. Same workspaces, same pixels, no Electron.

notslack is a native Slack client written in Rust on [GPUI](https://www.gpui.rs), the GPU-accelerated UI framework that powers the Zed editor. It signs in with the Slack session you already have and looks exactly like the app you use every day, except it is a single native binary drawing straight to the GPU.

## Download

| Platform | Get it |
| --- | --- |
| macOS, Apple Silicon | [notslack-macos-arm64.dmg](https://github.com/nothq/notslack/releases/latest/download/notslack-macos-arm64.dmg) |
| macOS, Intel | [notslack-macos-x86_64.dmg](https://github.com/nothq/notslack/releases/latest/download/notslack-macos-x86_64.dmg) |
| Linux, x86_64 | [notslack-linux-x86_64.tar.gz](https://github.com/nothq/notslack/releases/latest/download/notslack-linux-x86_64.tar.gz) |
| Windows, x86_64 | [notslack-windows-x86_64.zip](https://github.com/nothq/notslack/releases/latest/download/notslack-windows-x86_64.zip) |

Each download is the whole app: one native binary plus the FFmpeg libraries it plays video with. No installer, no runtime, nothing else to fetch.

- **macOS**: open the .dmg and drag notslack into Applications. The first time you open it, go to System Settings › Privacy & Security and click **Open Anyway**.
- **Linux**: `tar -xzf notslack-linux-x86_64.tar.gz` and run `./notslack-linux-x86_64/notslack`.
- **Windows**: unzip and run `notslack.exe`.

Every [release](https://github.com/nothq/notslack/releases) is built from source by GitHub Actions.

## Why

Slack Desktop is Electron: a whole copy of Chromium plus Node.js, running a web page per workspace, split across a swarm of helper processes. All of that to show you text.

notslack throws the browser away. No DOM, no JavaScript, no garbage collector. Every pixel is Rust rendered straight to the GPU through Metal, Vulkan or DirectX, so it opens fast, scrolls at your display's refresh rate and barely registers in Activity Monitor. And it is pixel perfect: the sidebar, message list, threads, composer, reactions and search match Slack, so there is nothing to relearn.

| Same workspaces, 2560×1722 window | Memory footprint |
| --- | --- |
| Slack Desktop (8 processes) | ~1.16 GB |
| **notslack** (1 process) | **~165 MB** |

About 70 MB of notslack's number is the floor for any GPUI window that size, mostly GPU frame buffers. Measured with macOS's `footprint` tool on a MacBook Pro, 4 October 2026.

## Come build it

This is early, and that is the fun part. Desktop notifications, Wayland, signed and notarized builds, and every place where we are a pixel off from Slack are all open. If you have ever watched Slack eat your laptop's memory, or wanted to ship real code on GPUI, pick an issue and open a PR.

## What's there

- Every workspace you are signed in to in Slack Desktop, with the workspace switcher
- Channels, DMs, group DMs, sidebar sections, unreads and mentions
- Threads, reactions with the emoji picker, edits, drafts and scheduled messages
- Search, Activity, Later, Files and channel details
- Images, video and audio playback, file uploads, and audio and video clips from the composer
- Live updates over Slack's real-time connection
- Light and dark mode, following the system

## How sign-in works

notslack reuses the session of the Slack Desktop app you already have. The first time you click **Connect Slack**, notslack quits Slack Desktop, relaunches it with a local debugging port, reads the session of each signed-in workspace, and stores it in `~/.notslack/auth.json`, readable only by you. Nothing is sent anywhere except to Slack.

So you need Slack Desktop installed and signed in. That works on macOS, Windows (Slack's installer or the MSI) and Linux (the .deb, .rpm, Snap or Flatpak).

## Build and run

You need Rust 1.95 or newer and FFmpeg 8 with `pkg-config` (used for video playback and clip recording). The first build compiles GPUI and takes a few minutes.

On macOS, with the Xcode command line tools:

```bash
xcode-select --install
brew install ffmpeg pkg-config
git clone https://github.com/nothq/notslack
cd notslack
cargo run --release
```

On Linux, build the same FFmpeg the releases ship, then run against it:

```bash
sudo apt install nasm pkg-config clang libasound2-dev libfontconfig-dev libvulkan-dev \
  libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev
script/build-ffmpeg ~/.local/notslack-ffmpeg
export PKG_CONFIG_PATH=~/.local/notslack-ffmpeg/lib/pkgconfig LD_LIBRARY_PATH=~/.local/notslack-ffmpeg/lib
cargo run --release
```

On Windows, unpack an FFmpeg 8 shared build such as [BtbN's](https://github.com/BtbN/FFmpeg-Builds/releases), install LLVM (bindgen needs libclang), set `FFMPEG_INCLUDE_DIR`, `FFMPEG_LIBS_DIR` and `FFMPEG_LINK_MODE=dynamic`, put its `bin` folder on `PATH`, and `cargo run --release`.

`script/package` turns a release build into the downloads above, and `.github/workflows/release.yml` runs the whole thing for every platform when a `v*` tag is pushed.

## Layout

| Crate | What it does |
| --- | --- |
| `crates/notslack` | The app: opens the window and hosts the chat surface |
| `crates/chat` | The Slack client: API and real-time connection (`live`), data types (`model`) and the GPUI interface (`ui`) |
| `crates/gpui-components` | Text input, selectable text and other shared widgets |
| `crates/app/model` | Theme, appearance and the surface interface |
| `crates/local_cache`, `crates/secret_store` | Encrypted on-disk cache and the credential file |
| `crates/media/capture`, `crates/video/*` | Microphone and camera capture, FFmpeg video playback |

## Disclaimer

notslack is an independent project. It is not affiliated with or endorsed by Slack Technologies or Salesforce.

## License

[AGPL-3.0](LICENSE). The bundled Lato fonts are under the [SIL Open Font License](crates/notslack/assets/fonts/OFL.txt).
