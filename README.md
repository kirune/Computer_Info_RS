# PRH Computer Info (Rust)

A lightweight desktop application that displays local computer and user information, and provides quick-access tools for Pullman Regional Hospital staff.

This is the Rust rewrite of [PRH_Computer_Info](https://github.com/kirune/PRH_Computer_Info) (WPF / .NET 8), with the same features, look, and CI/CD pipeline — compiled to a single dependency-free native executable.

![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust&logoColor=white)
![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)

## Features

- Display computer name, IP addresses, logged-in user, OS, default printer, and reboot time
- One-click buttons to:
  - Copy system info to clipboard
  - Auto-generate email to IT with system info prefilled
  - Access Epic help resources
- Add a shared printer from the print server (24h cached list, live search, optional set-as-default)
- PRH-branded look and feel

## Requirements

- Windows 10 or later (x64)

That's it. Unlike the WPF version, there is no runtime dependency — the Rust build is always self-contained, so the old framework-dependent vs. self-contained split is gone. Each release publishes:

- **`ComputerInfo.exe`** — single ~5 MB native executable, runs standalone
- **`ComputerInfo-<version>-win-x64.zip`** — the same executable, zipped

## Building

```powershell
cargo build --release
```

The exe lands at `target\release\computer_info.exe`. The UI and shared logic also compile and run on Linux (with stubbed platform calls) for development.

PRH branding assets (`Assets\AppIconNew.ico`, `Assets\prh_logo.png`) are vendored in this repository for build convenience; per the license terms below, the branding itself is excluded from the GPL-3.0 grant. Builds without them (e.g. if `Assets\` is removed) produce a fully functional unbranded app.

## Release process

1. Bump `version` in `Cargo.toml`
2. Tag `vX.Y.Z` and push the tag
3. CI verifies the tag matches `Cargo.toml`, builds, publishes the GitHub Release, and commits the internal winget manifest (`PRH.ComputerInfo` — same identifier as the WPF app, so winget treats it as an upgrade)

## License

This project is licensed under the [GNU General Public License v3.0 (GPL-3.0)](LICENSE).

### Additional Terms

In accordance with Section 7 of GPL-3.0:

- **Logos and Trademarks**
  The Pullman Regional Hospital (PRH) name, logo(s), and any associated trademarks are *not* licensed under GPL-3.0.
  These marks remain the exclusive property of PRH.

- **Redistribution**
  Any redistribution of this software outside of PRH must remove or replace the PRH name and logo(s).
  Branding assets are provided for internal PRH use only and may not be redistributed or reused.

- **No Endorsement**
  The presence of PRH branding in this software does not imply endorsement, certification, or responsibility for modifications made by third parties.
