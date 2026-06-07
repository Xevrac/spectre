<img width="1024" height="420" alt="spectre-banner" src="https://github.com/user-attachments/assets/dd2a5dac-0b8d-415b-ae7e-5d9437698334" />

# Spectre
Spectre is a work in progress toolkit for Hidden &amp; Dangerous 2.

## System requirements (Windows)

- **Windows 10 or later (64-bit)**
- **WebView2 Runtime** — [install](https://developer.microsoft.com/en-us/microsoft-edge/webview2/#download-section) if the app won’t start (Windows 11 has it; many Windows 10 builds don’t).
- **Graphics** — On Windows: wgpu (DX12) with LowPower. If no adapter is found, the app tries in order: wgpu+OpenGL, WARP, OpenGL (Glow), then the CPU software renderer.
- **VC++ Redistributable (x64)** — Usually present; [install](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) if you see DLL errors.

# Features
## Tools
### Server Utility
A single place to configure, run and monitor HD2 and Sabre Squadron dedicated servers. Inspired by Server Manager.

- **Server & profiles** — Add multiple servers (ports), each with config profiles (session name, style, game rules, map rotation). Switch between HD2 and Sabre Squadron per server.
- **Players** — See who’s connected, ban by IP (with optional in-game reason), and manage a whitelist.
- **Watchdog** — Restart crashed servers automatically; optionally restart all servers on a schedule (e.g. every N days). When scheduled restart is enabled, in-game countdown messages (T-5min, T-1min, T-10s, restarting now) can be sent before the restart.
- **Automated announcements** — Broadcast messages in-game at a set interval. Use one shared list for all servers or a separate list per server. Messages are sent via `asay` (43-character limit).
- **Logs** — View daemon and Spectre events per server.
- **Settings** — Session/game options, coop settings, passwords, and log rotation. Config is saved to JSON and survives restarts.

Requires Spectre to be running for watchdog and automated features. One local server can run at a time per Spectre instance.

- DTA Unpacker (Planned)

## Editors
- Inventory (Planned)
   - Edit your save game inventories with ease
- Items (Planned)
   - Tweak values, edit or create items for the game
- MP Maplist (Planned)
   - An improved maplist constructor
- Gamedata (Planned)
   - A gamedata editor allowing for campaign modifications

## Media
### Server Utility
<img width="2549" height="1267" alt="image" src="https://github.com/user-attachments/assets/d92e66c7-3129-49fc-a3a2-3048a8425a98" />

<img width="2551" height="1263" alt="image" src="https://github.com/user-attachments/assets/fe327b8f-3773-4e72-85e0-317b46f49093" />

<img width="2549" height="1263" alt="image" src="https://github.com/user-attachments/assets/719d0a90-5a71-410b-94a0-1d0ad725626a" />

<img width="2540" height="1263" alt="image" src="https://github.com/user-attachments/assets/1685cf91-97f8-42ea-a538-395869258e02" />

# Credits
A special thanks to those who have worked on previous projects, research, sharing knowledge, source code and supported the HD2 community. Notably:
- Fis
- Stern
- snowmanflo
- Jovan Stanojlovic
- RellHaiser
