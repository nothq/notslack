# notslack

A native Slack client written in Rust with [GPUI](https://www.gpui.rs), the GPU-accelerated UI framework behind the Zed editor.

## Why

Slack's desktop app is Electron: a full copy of Chromium plus a Node.js runtime, running a web page per workspace. It is slow to start, it stutters when you scroll a busy channel, and it holds on to huge amounts of RAM to show you text.

notslack draws the same interface natively. There is no browser, no JavaScript and no DOM. The UI is Rust rendered straight to the GPU through Metal, so it starts fast, scrolls smoothly and uses a fraction of the memory. It is built to be pixel perfect: the sidebar, message list, threads, composer, reactions and search match the Slack app you already know, so there is nothing to relearn.

On a MacBook Pro with a 2560×1722 window and the same Slack workspaces signed in, Slack Desktop's eight processes had a combined memory footprint of about 1.16 GB, while notslack settled at about 165 MB, roughly a seventh. About 70 MB of that is the cost of any GPUI window at that size (mostly the GPU frame buffers). Measured with macOS's `footprint` tool on 4 October 2026; Slack had been running for two days and notslack for two minutes, so the gap at equal uptime may be smaller.

It is early and there is a lot to do. If you have ever watched Slack eat your laptop's memory, come help.

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

So you need Slack Desktop installed and signed in, and for now that means macOS.

## Build and run

You need macOS, the Xcode command line tools, Rust 1.95 or newer, and FFmpeg 8 with `pkg-config` (used for video playback and clip recording).

```bash
xcode-select --install
brew install ffmpeg pkg-config
git clone https://github.com/nothq/notslack
cd notslack
cargo run --release
```

The first build compiles GPUI and takes a few minutes.

## Layout

| Crate | What it does |
| --- | --- |
| `crates/notslack` | The app: opens the window and hosts the chat surface |
| `crates/chat` | The Slack client: API and real-time connection (`live`), data types (`model`) and the GPUI interface (`ui`) |
| `crates/gpui-components` | Text input, selectable text and other shared widgets |
| `crates/app/model` | Theme, appearance and the surface interface |
| `crates/local_cache`, `crates/secret_store` | Encrypted on-disk cache and the credential file |
| `crates/media/capture`, `crates/video/*` | Microphone and camera capture, FFmpeg video playback |

## Contributing

Issues and pull requests are welcome. Good places to start are Linux and Windows support, desktop notifications, and anything that looks or behaves differently from Slack.

notslack is an independent project. It is not affiliated with or endorsed by Slack Technologies or Salesforce.

## License

[AGPL-3.0](LICENSE). The bundled Lato fonts are under the [SIL Open Font License](crates/notslack/assets/fonts/OFL.txt).
