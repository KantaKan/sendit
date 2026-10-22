# sendit v1 design

Working name: **sendit**. Open-source, LocalSend-compatible desktop file transfer app with a new UI and stronger LAN identity checks.

## Goals

- Talk LocalSend protocol v2 so sendit and the official LocalSend apps (phone or desktop) can send files to each other in both directions.
- Have a better UX than LocalSend: you send by dropping files, with no tab-then-pick-file-then-pick-device flow.
- Defend against impersonation and MITM on a shared LAN, which LocalSend leaves open.
- Learning project: implement the protocol by hand in Rust. Ships open source and gets posted on Reddit (r/rust, r/selfhosted).

## Non-goals (v1)

- Transfers over the internet or a relay. sendit is LAN only.
- Mobile apps. Phones use the official LocalSend app.
- Radar view, Compact/tray view, QR web share, Thai UI, a full transfer-history browser, QUIC transport. All of these are planned after v1.
- Using "LocalSend" in the product name or branding. It is described as "LocalSend-compatible".

## Architecture

Cargo workspace monorepo:

```
sendit/
├─ crates/core/   protocol library, no UI
│   ├─ identity    self-signed cert (rcgen), fingerprint = SHA-256 of cert DER
│   ├─ discovery   UDP multicast 224.0.0.167:53317 + HTTP /register subnet scan fallback
│   ├─ server      HTTPS server (axum + rustls) on TCP 53317
│   ├─ client      outgoing transfers (reqwest + rustls)
│   ├─ trust       TOFU store: alias/fingerprint pins, verified flag
│   └─ events      Event enum broadcast to UI consumers
├─ crates/cli/    `sendit send <paths> --to <device>`, `sendit receive`, `sendit devices` (clap)
└─ app/           Tauri 2. src-tauri depends on core; frontend is Svelte + TypeScript
```

`core` exposes an async API (`Node::start(config)`, `node.send(fingerprint, paths, pin)`, `node.respond(request_id, decision)`, `node.cancel(transfer_id)`) and an event stream (`DeviceUpdated`, `IdentityChanged`, `IncomingRequest`, `TransferStarted`, `Progress`, `TransferDone`, `TransferFailed`). There is no "device lost" event, because LocalSend peers don't re-announce periodically. "Nearby" means the device was seen this session. The GUI and the CLI are both thin consumers of that API and stream.

Transport: HTTP over TLS over TCP. LocalSend compatibility requires this. Discovery runs over UDP multicast.

## Protocol (LocalSend v2, implemented to spec)

Reference: https://github.com/localsend/protocol

- Announce over multicast: `{alias, version, deviceModel, deviceType, fingerprint, port, protocol:"https", download, announce:true}`. A peer that hears an announce replies with `POST /api/localsend/v2/register`, falling back to a UDP reply with `announce:false`.
- Fallback discovery: `POST /api/localsend/v2/register` to every host on the local /24 subnets.
- Send: `POST /api/localsend/v2/prepare-upload` (`?pin=` optional) returns `{sessionId, files:{fileId:token}}`. Then one `POST /api/localsend/v2/upload?sessionId&fileId&token` per file with a binary body, run in parallel. `POST /api/localsend/v2/cancel?sessionId` aborts.
- Status codes are handled exactly as the spec defines them: 204, 400, 401 (PIN), 403 (rejected), 409 (busy), 422 (checksum), 429, 500.
- `deviceType` is `desktop`. `version` is whatever the protocol README pins at implementation time, currently 2.x.
- The download API (`prepare-download`/`download`) is out of v1 scope. It becomes the base for the QR web share feature.
- LocalSend devices with encryption turned off (`protocol: "http"`) are ignored in v1, because there is no cert to pin.
- Verifying an incoming sender: before a request is shown, sendit calls the sender's `/register`, pinned to the fingerprint the sender claimed. If that succeeds, the request is marked verified. Otherwise it is shown as "unverified".
- sendit does not send `sha256` for outgoing files, because hashing large files first would delay the start. When a peer does send `sha256`, sendit checks it on receipt.

## Security

| Threat | Mitigation |
|---|---|
| A peer announces another device's alias | TOFU: the fingerprint is pinned per device on first contact. If the same alias shows up with a new fingerprint, the UI shows flow C and nothing is sent until the user decides. |
| MITM on the TLS connection | The fingerprint of the presented cert must equal the fingerprint that was announced or registered. On mismatch the connection is aborted. A custom rustls `ServerCertVerifier` replaces CA validation. |
| First contact cannot be verified by TOFU | Between sendit peers, both screens show the same avatar pair, derived from both fingerprints, and the user compares them (flow B). LocalSend peers skip this step and are labelled "unverified". |
| Unsolicited files | Every incoming request needs an explicit Accept, unless the sender is a verified device marked auto-accept. A PIN can be turned on. |
| Path traversal / hostile filenames | Each `/`- or `\`-separated component is cleaned separately, so folder sends keep their structure. `.`, `..`, empty and absolute-path components are dropped. Reserved Windows names, `<>:"|?*`, control characters and bidi-override characters are escaped or replaced. The final path must canonicalize to a location inside the chosen destination directory. |
| Disk exhaustion | Total size is checked against free space before Accept is offered. Uploads that send more bytes than they declared are aborted. |

Device avatar: a DiceBear `thumbs` avatar rendered offline with `@dicebear/core` + `@dicebear/collection`. The seed is a hex prefix of the fingerprint, so if a device's key changes, its avatar visibly changes too.

## UX (v1)

Visual direction "Ticket v3" (mockups in `.superpowers/brainstorm/`, not committed):

- Dark theme. Background `#111113`, surfaces `#1e1e22`/`#222226`, text `#ececea`, a single accent `#3ecf8e`, danger `#e5533d`.
- Familjen Grotesk for UI text, JetBrains Mono for metadata (sizes, IPs, speeds). Fonts are bundled locally.
- No stock icon set and no emoji. File types appear as text (`JPG`, `PDF`). Devices are shown with their DiceBear thumbs avatar.
- UI copy is English. Strings live in one module so a Thai locale can be added later.

Layout: the sidebar lists devices, split into "nearby" and "seen before". The main pane is a conversation with the selected device, where every transfer is a ticket (notched stub on the right). Incoming tickets sit left-aligned and are neutral. Outgoing tickets sit right-aligned and are green. A live ticket fills with the accent color as it progresses. There is no separate progress bar track.

Received files are only revealed in the file manager ("Show in folder"), never opened from the app, so one click can't run a file a peer sent.

Flows:
- **Send**: drop files anywhere in a conversation, or use the "Choose files" button. The transfer starts at once and appears as a live ticket. Pasting files with Ctrl/Cmd+V is deferred: the webview only receives file contents, not paths.
- **A. Incoming**: a dashed "wants to send N files" ticket shows up in that device's conversation with Accept / Decline and a destination link. An OS notification fires if the window is not focused. There is no full-screen takeover.
- **B. First sendit contact**: an avatar-pair comparison with "Yes, trust" / "Skip".
- **C. Identity changed**: a red inline panel that shows the old and new fingerprint prefixes, with "Block" / "It's a reinstall, trust new key".

States the UI must have: empty (no devices yet: explain that devices on the same Wi-Fi appear here), searching, failed ticket with Retry, declined, cancelled.

## Error handling

- **Interrupted transfer**: the ticket moves to `failed` and shows the reason (peer gone, declined, checksum, disk). Retry resends the whole file, because the protocol has no resume.
- **Receiving**: data is written to `<name>.part` and renamed on completion. Any failure or cancel deletes the `.part` file.
- **Name collisions**: an existing file is never overwritten. A suffix is added: `name (1).ext`.
- **Stalled transfer**: both sides fail the transfer if no bytes move for 30 s. This also frees the receiver's single session slot when a sender disappears right after `prepare-upload`.
- **Network change** (IP or interface changes): the multicast socket is rebuilt and a new announce goes out. Known devices stay in the "seen before" list.
- **Port 53317 already in use** (for example, LocalSend is also running): the UI says so clearly. Falling back to another port still works for sending, because peers learn the port from the announce.
- **Errors do not panic or crash the app**: `core` errors are typed with `thiserror` and mapped to UI copy in one place.

## Testing

1. **Unit**: filename sanitizer, fingerprint computation, protocol JSON round-trips against fixtures copied from the protocol README, TOFU state machine.
2. **Property/fuzz**: `proptest` for the sanitizer (no output escapes the destination dir) and JSON round-trips. `cargo-fuzz` targets for the request DTO parsing and the sanitizer.
3. **Integration**: two `Node`s inside one process on localhost send files of 0 B, 1 B, 10 MB and 1 GB (generated) plus multi-file batches. Received files must match by sha256.
4. **Security scenarios**: a fake peer that reuses a pinned alias with a new key gets `IdentityChanged` and no data is sent. A cert/fingerprint mismatch aborts the TLS connection. Path traversal names never leave the destination directory.
5. **Network chaos**: a small TCP proxy in the test harness that drops connections mid-stream, throttles bandwidth and stalls. Tests assert the `.part` file is removed, the transfer reaches `failed`, and Retry succeeds.
6. **UI**: Vitest for Svelte components (ticket states, device list). Playwright runs flows A/B/C and send/retry against the frontend with a mocked event bridge.
7. **Real interop (manual checklist)**: send to and receive from LocalSend on Android, iOS and desktop. Also check PIN, decline and cancel from each side.
8. **CI**: GitHub Actions runs the full matrix on Windows, macOS and Linux (fmt, clippy, unit/property/integration/chaos, UI tests). Fuzzing runs in a short nightly job.
9. **Benchmark**: a throughput run (MB/s) for sendit to sendit and sendit to LocalSend on the same LAN. Results and the method go in the README.

## Done criteria for v1

- Every item in the interop checklist passes against official LocalSend.
- Automated test layers 1-6 pass in CI on all 3 OSes.
- Unsigned release builds for Windows, macOS and Linux are attached to a GitHub release (code signing is out of v1 scope), and the README includes a demo GIF.

## Repo hygiene

- `.superpowers/` is gitignored.
- Commits carry only the author's identity. No tool attribution trailers.
- License: Apache-2.0 (same as LocalSend).
