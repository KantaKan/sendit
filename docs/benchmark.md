# Benchmark method

1. Loopback (CPU/TLS ceiling): `cargo run --release -p sendit-core --example loopback_bench -- 1024`
2. LAN, sendit -> sendit: two machines on the same router (wired if possible).
   Receiver: `sendit receive --yes`. Sender: `sendit send <1 GB file> --to <name>`. Note the final MB/s.
3. LAN, sendit -> LocalSend and LocalSend -> sendit: same file, same machines, LocalSend app on the other side.
   Time from accept to done with a stopwatch, 3 runs, take the median.

Record date, OS + version, network (Wi-Fi standard or wired), and median MB/s per direction.

| Date | Setup | Loopback | sendit->sendit | sendit->LocalSend | LocalSend->sendit |
|---|---|---|---|---|---|
| 2026-10-23 | Windows 11, debug-free release build, 512 MB | 722 MB/s | | | |
