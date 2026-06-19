<div align="center">

# 🧹 CleanMyLinux

**A native Linux system-cleanup app with a CleanMyMac-style experience — for both GNOME and KDE.**

Smart Scan · System Junk · Package Cleanup · Large & Old Files · App Uninstaller · System Monitor

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange)
![GTK4](https://img.shields.io/badge/GNOME-GTK4%20%2B%20libadwaita-4A86CF)
![Kirigami](https://img.shields.io/badge/KDE-Qt6%20%2B%20Kirigami-1D99F3)

</div>

---

## What is it?

CleanMyLinux reclaims disk space and keeps your system tidy through a polished,
one-click experience inspired by CleanMyMac — but built with **native Linux
toolkits** so it feels at home on your desktop, not bolted on.

- 🔍 **Smart Scan** — one click finds reclaimable space across every category, shown on a big circular gauge.
- 🗑️ **System Junk** — user & app caches, thumbnails, Trash, old logs, browser caches.
- 📦 **Package Cleanup** — apt cache, orphaned packages, old kernels, snap revisions, unused flatpak runtimes.
- 📁 **Large & Old Files** — find space hogs across your home folder; recoverable via Trash.
- 🧩 **App Uninstaller** — remove apt / flatpak / snap apps cleanly.
- 📊 **System Monitor** — live CPU, RAM, swap and disk usage.

## Native on both GNOME *and* KDE

The entire scanning/cleanup engine lives in one Rust crate (`cml-core`) with **two
thin native frontends** on top:

| Desktop | Frontend | Toolkit |
| --- | --- | --- |
| **GNOME** | `cleanmylinux` | Rust + [relm4](https://relm4.org/) + GTK4 + **libadwaita** |
| **KDE** | `cleanmylinux-qt` | Rust + [cxx-qt](https://kdab.github.io/cxx-qt/) + Qt6/QML + **Kirigami** |

Same engine, same results, each desktop's real native look.

## Privacy & safety

- **No telemetry, no network calls.** Ever.
- **Nothing is auto-deleted** — every action shows exactly what and how much, and asks first.
- **Protected-paths safelist** — keyrings, SSH/GPG keys, and browser *profiles* are never touched (only their cache subfolders).
- **User files go to Trash** (recoverable); only regenerable junk is deleted permanently.
- **Least privilege** — root-only operations run through a small, audited polkit helper that accepts only a fixed whitelist of actions.

## Architecture

```
crates/
  cml-core/    # shared engine: scanning, cleaning, safety, config (no UI, unit-tested)
  cml-gtk/     # GNOME frontend  → binary `cleanmylinux`
  cml-qt/      # KDE frontend    → binary `cleanmylinux-qt`
  cml-helper/  # privileged helper → /usr/libexec/cleanmylinux-helper (polkit/pkexec)
data/          # .desktop files, icons, AppStream metadata, polkit policy, CSS
```

## Building from source

### Prerequisites (Ubuntu / Debian)

```bash
# GNOME frontend
sudo apt install -y libgtk-4-dev libadwaita-1-dev libglib2.0-dev

# KDE frontend (optional)
sudo apt install -y qt6-base-dev qt6-declarative-dev \
                    qml6-module-org-kde-kirigami libkf6kirigami-dev kirigami2-dev

# Rust toolchain (if not already installed): https://rustup.rs
```

### Run

```bash
cargo run -p cml-gtk      # GNOME (libadwaita)
cargo run -p cml-qt       # KDE   (Kirigami)
cargo test -p cml-core    # engine unit tests
```

### Package (.deb)

```bash
cargo install cargo-deb
cargo deb -p cml-gtk      # cleanmylinux-gnome_*.deb
cargo deb -p cml-qt       # cleanmylinux-kde_*.deb
```

## Status

🚧 Early development. See [the implementation plan](#) for the roadmap. Contributions welcome.

## License

[GPL-3.0-or-later](LICENSE) © CleanMyLinux contributors
