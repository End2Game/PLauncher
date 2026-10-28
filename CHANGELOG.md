# Changelog

## 1.7.0
- Added recursive installation of required CurseForge dependencies with cycle detection and dependency-first order.
- Updated Mod Center to report the number of installed mod and required dependency files.
- Added loader-profile-aware launch argument selection for Fabric, Quilt, Forge and NeoForge.
- Added additional standard launcher placeholders and corrected legacy argument fallback.
- Added native library extraction from both vanilla and loader metadata.
- Added Maven-coordinate fallback for native classifier paths.

## 1.6.0
- Forge official installer workflow and profile registration.
- NeoForge artifact availability check and profile detection.
- CurseForge file search, compatible file selection, download and checksum validation.
