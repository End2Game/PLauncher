# PLauncher 1.7.0 test checklist

These tests must be run on a Windows machine with the matching Java runtime and a clean Minecraft directory. The current artifact-generation environment did not contain Rust tooling or a Minecraft installation, so none of the game-launch tests below are claimed as passed.

## Build checks
1. Install Rust stable and run `cargo fmt --check`.
2. Run `cargo check`.
3. Run `cargo test`.
4. Run `cargo run --release`.

## Loader installation and launch matrix
For each row, install the Vanilla version first, install the exact loader version, then launch using both Offline mode (for local client startup only) and a valid Microsoft account. Online-mode multiplayer requires a legitimate authenticated account.

| Loader | Suggested test target | Required checks |
|---|---|---|
| Vanilla baseline | Minecraft 1.20.1 + Java 17 | Client starts, title screen appears, log has no missing class/library errors |
| Forge | Minecraft 1.20.1 + a compatible Forge 47.x build + Java 17 | Installer creates profile, loader libraries resolve, client starts |
| NeoForge | Minecraft 1.21.1 + matching NeoForge 21.1.x + Java 21 | Installer creates profile, native libraries load, client starts |
| Fabric | Minecraft 1.20.1 + current compatible Fabric Loader + Java 17 | Loader main class/arguments resolve, client starts |
| Quilt | Minecraft 1.20.1 + compatible Quilt Loader + Java 17 | Loader main class/arguments resolve, client starts |

## CurseForge dependency tests
1. Use a mod with one required dependency (`relationType = 3`).
2. Confirm the dependency is installed before the selected mod.
3. Test a two-mod dependency cycle using mocked API responses; installation must stop with a clear error.
4. Test optional and incompatible dependencies; they must not be auto-installed.
5. Test a missing dependency file and a failed checksum; the operation must report an error and must not leave a partial JAR.

## Evidence to capture
- `logs/latest-launch.log` for every loader.
- Installer stdout/stderr for Forge and NeoForge failures.
- The exact Minecraft, loader and Java versions.
- A screenshot of the game reaching the title screen.
