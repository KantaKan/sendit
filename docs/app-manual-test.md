# App manual test (run before a release)

Setup: `cd app && npm run tauri dev`, plus a phone with LocalSend on the same Wi-Fi.

- [ ] Phone appears under "nearby" within a few seconds (press Refresh if not)
- [ ] Drop a file on the window: ticket fills, ends with DONE, phone has the file
- [ ] "Choose files" sends the picked files
- [ ] Phone sends 3 photos: dashed request lists them; Accept; tickets fill; "Show in folder" reveals the file
- [ ] Phone sends while the window is minimized: a system notification appears
- [ ] "to Downloads/sendit" lets you pick another folder and saves there
- [ ] Decline on a request: the phone reports it was declined
- [ ] Turn Wi-Fi off mid-transfer: ticket shows FAILED, "Connection lost", Retry works after Wi-Fi returns
- [ ] Two sendit computers: verify panel shows the same two faces on both; "Yes, trust" marks the device verified
- [ ] LocalSend desktop open on the same PC: the port banner appears and transfers still work
- [ ] Keyboard only: Tab reaches every button and a visible focus ring shows
