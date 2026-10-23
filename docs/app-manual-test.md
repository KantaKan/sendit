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

## v0.2: tray, sign-in, single instance (needs a real desktop)

- [ ] Tray shows the sendit icon with the tooltip "sendit". Left-click hides the window, and a second left-click shows it.
- [ ] Tray menu: *Open sendit* shows the window, *Quit* exits (port 53317 is free afterwards).
- [ ] With "Keep running in tray": X hides the window, and the first time a notification says sendit is still running. A phone can still send. The incoming request shows a notification.
- [ ] With "Quit": X exits the app.
- [ ] Start on sign-in on: sign out and back in. sendit is in the tray and the window is not shown. Turn it off: it no longer starts.
- [ ] Launch sendit a second time while it runs: the existing window comes to the front, and no second tray icon appears.
- [ ] Rename the device in Settings: the phone's LocalSend list shows the new name after a refresh.
- [ ] Change the download folder, then receive a file: it lands in the new folder. Restart: the folder is remembered.
- [ ] Quit and relaunch: finished transfers and messages are still in each conversation.
