# PLauncher 1.7.0

**PLauncher** is an experimental Minecraft launcher written in Rust with egui/eframe. This repository contains source code and a GitHub Actions workflow that builds a Windows x64 release package.

> **Development build:** this version has not yet been compiled and tested end-to-end on a Windows machine with real Minecraft installations and Microsoft authentication. Loader compatibility is not guaranteed. Review [TESTING.md](TESTING.md) before relying on it.

## Download for Windows

1. Open the repository's **Releases** page.
2. Open the newest release.
3. Download `PLauncher-<version>-Windows-x64.zip` from **Assets**.
4. Extract the ZIP and run `PLauncher.exe`.

Do not download the GitHub **Source code** ZIP if you only want to run the launcher; download the Windows x64 asset.

## Build locally (Windows 10/11 x64)

Install [Rust](https://rustup.rs/) and Visual Studio Build Tools with the **Desktop development with C++** workload. Then open PowerShell in this repository and run:

```powershell
rustup toolchain install stable
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc
```

The executable is created at:

```text
target\x86_64-pc-windows-msvc\release\plauncher.exe
```

The executable's actual filename is determined by the Cargo package/binary name (`plauncher.exe`).

## GitHub release automation

The workflow in `.github/workflows/windows-release.yml` runs when a tag beginning with `v` is pushed. It builds the Windows x64 executable on GitHub-hosted Windows, packages it with this README, and attaches the ZIP to a GitHub Release.

To publish a release:

```powershell
git add .
git commit -m "Prepare PLauncher 1.7.0 release"
git push origin main
git tag v1.7.0
git push origin v1.7.0
```

Then open **Actions** in GitHub to monitor the build, and **Releases** to get the published ZIP. The workflow needs the repository's default `GITHUB_TOKEN` permissions; no personal access token is required.

## Project documents

- [CHANGELOG.md](CHANGELOG.md) — changes by version
- [ROADMAP.md](ROADMAP.md) — planned work
- [TESTING.md](TESTING.md) — validation checklist
- [LICENSE](LICENSE) — MIT license

## Important

This project is not affiliated with Mojang or Microsoft. Minecraft is a trademark of Mojang Studios. Users are responsible for complying with Minecraft's terms and the terms of any mod platform they use.
