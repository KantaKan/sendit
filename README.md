# sendit

LocalSend-compatible file transfer for Windows, macOS and Linux. It talks to the LocalSend apps on your phone, and it pins every device's key so nobody on the same Wi-Fi can pretend to be your laptop.

## CLI

    sendit devices
    sendit send photo.jpg album/ --to "Pixel 8"
    sendit receive

## Build

    cargo build --release

## Why another LocalSend client?

- New desktop UI (coming in the app)
- Trust on first use: a device that reuses a known name with a new key is blocked until you confirm it
- A scriptable CLI

Protocol: https://github.com/localsend/protocol. Not affiliated with LocalSend.

License: Apache-2.0
