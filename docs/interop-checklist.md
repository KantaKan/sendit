# Interop checklist (run before every release)

Peers: LocalSend on Android, iOS, and desktop (Windows or macOS). Encryption ON (LocalSend default).

For each peer:
- [ ] `sendit devices` lists the peer, and the peer lists sendit
- [ ] sendit -> peer: single file, accepted on peer, arrives intact
- [ ] sendit -> peer: a folder with a subfolder keeps its structure
- [ ] sendit -> peer: declined on peer gives CLI "Declined"
- [ ] sendit -> peer with PIN set on peer: without --pin fails, with --pin works
- [ ] peer -> sendit: single file and multi-file batch arrive under the download dir
- [ ] peer -> sendit: decline in sendit is reported as declined on peer
- [ ] peer -> sendit: cancel on peer mid-transfer leaves no `.sendit-part` file
- [ ] sendit cancel (Ctrl+C on send) mid-transfer, then peer accepts a new transfer immediately
- [ ] LocalSend app open on the same PC as sendit: sendit reports port fallback and still sends
- [ ] The extra `sendit` field in our JSON does not break the peer
