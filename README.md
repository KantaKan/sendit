# sendit

**LocalSend-compatible file transfer for Windows, macOS and Linux.** It sends files, folders, screenshots and text to the LocalSend app on your phone (and to other sendit computers) over your Wi-Fi. No cloud, no account.

![sendit demo](docs/media/demo.gif)

<sub>Higher quality: [demo.mp4](docs/media/demo.mp4)</sub>

| Finding devices | Receiving | History |
|---|---|---|
| ![discovery](docs/media/discovery.png) | ![receiving](docs/media/receiving.png) | ![timeline](docs/media/timeline.png) |

## What it does

- **Works with LocalSend.** Phones and computers that run LocalSend (protocol v2, including 2.2's mutual TLS) see sendit and can trade files with it in both directions.
- **Drop or paste to send.** Drag files into the window, or press <kbd>Ctrl</kbd>/<kbd>Cmd</kbd>+<kbd>V</kbd> to send whatever you copied: files, folders, a screenshot, or text. Text shows up on the phone as a message.
- **A timeline, not a progress dialog.** Incoming requests, live transfers (with a speed graph and time left) and finished items with photo thumbnails. Right-click any item to show it in its folder, copy it, retry it, or remove it.
- **Harder to fool than plain LocalSend.** Every device's key is pinned the first time you see it. If a device shows up later with a familiar name but a different key, sendit warns you and won't send until you decide. Two sendit computers can confirm each other by comparing a pair of faces.
- **Safe receiving.** You accept each request (or auto-accept trusted devices). Files never land outside your download folder, never overwrite anything, and are only ever *revealed* in your file manager, never opened.
- **A CLI too:** `sendit devices`, `sendit send <files> --to <name>`, `sendit receive`.

## Run it

You need [Rust](https://rustup.rs) and [Node.js](https://nodejs.org). On Windows, Rust must use the MSVC toolchain.

    git clone https://github.com/KantaKan/sendit
    cd sendit/app
    npm install
    npm run tauri dev        # the desktop app
    npm run dev:mock         # UI only, in a browser, with a fake backend

CLI:

    cargo run -p sendit-cli -- devices
    cargo run -p sendit-cli -- send photo.jpg album/ --to "iPhone"
    cargo run -p sendit-cli -- receive

Build an installer: `cd app && npm run tauri build`

If your phone doesn't show up, check that both are on the same Wi-Fi, and on iPhone keep LocalSend open on screen (iOS pauses it in the background).

## How it's built

A Rust core (`crates/core`) speaks the [LocalSend protocol](https://github.com/localsend/protocol) by hand: UDP multicast discovery, an HTTPS server and client on rustls with certificate pinning, and a trust-on-first-use store. The desktop app is Tauri 2 with Svelte 5. Tests cover the protocol, security cases (impersonation, path traversal, PIN brute force), network chaos (dropped, throttled and stalled links) and the UI flows in a real browser.

Design notes: [docs/design](docs/design). Interop checklist: [docs/interop-checklist.md](docs/interop-checklist.md).

The demo above is generated: `cd app && DEMO=1 npx playwright test e2e/demo.spec.ts`.

Not affiliated with LocalSend. License: Apache-2.0.
