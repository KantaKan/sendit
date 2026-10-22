# sendit

LocalSend-compatible file transfer for Windows, macOS and Linux. It talks to the LocalSend apps on your phone, and it pins every device's key so nobody on the same Wi-Fi can pretend to be your laptop.

## CLI

    sendit devices
    sendit send photo.jpg album/ --to "Pixel 8"
    sendit receive

## App

    cd app
    npm install
    npm run tauri dev        # run the desktop app
    npm run dev:mock         # UI only, in a browser, with a fake backend

## Build

    cargo build --release

## Why another LocalSend client?

- A desktop app where every transfer is a ticket in a chat with that device
- Trust on first use: a device that reuses a known name with a new key is blocked until you confirm it
- A scriptable CLI

Protocol: https://github.com/localsend/protocol. Not affiliated with LocalSend.

License: Apache-2.0
