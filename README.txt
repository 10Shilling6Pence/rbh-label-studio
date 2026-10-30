RBH Label Studio - Phase 4 Windows Application Identity

Based on the committed rbh-development branch (e56c669, compact startup splash).

Files:
  rbh-application-identity-phase4.patch
  rbh-app-icon.png             (transparent PNG)
  rbh-app-icon.ico             (multi-resolution Windows icon)

From repository root in PowerShell:
  git apply --check ".\rbh-application-identity-phase4.patch"
  git apply ".\rbh-application-identity-phase4.patch"
  Copy-Item ".\rbh-app-icon.png" ".\apps\vectorcraft\src\rbh-app-icon.png"
  Copy-Item ".\rbh-app-icon.ico" ".\apps\vectorcraft\src\rbh-app-icon.ico"
  cargo check -p vectorcraft -j 1
  cargo build -p vectorcraft -j 1
  .\target\debug\vectorcraft.exe

Changes: EXE resource icon & metadata, native window title, runtime window/taskbar icon.
Keeps binary name vectorcraft.exe and leaves upstream copyright/version notice unchanged.
Windows Explorer sometimes caches prior icon; inspect EXE Properties > Details or copy
EXE to a new filename for visual check if shell caching is stubborn.

This patch is not a wholesale UI rename. VectorCraft welcome/about screens and menus
are hosted by the upstream UI crate and need a separate scoped phase.
Do not commit until Windows build and icon/title verified.
