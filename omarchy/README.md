# Omarchy frontend

`deezer-player` is a Quickshell/Omarchy bar widget for the `deezer-tui`
daemon. It intentionally uses Omarchy's `Color` and `Style` singletons rather
than fixed colours, fonts, or spacing, so every system theme is reflected
immediately in the player UI.

## Install for development

Build or install `deezer-tui`, then copy the plugin into your user-owned
Omarchy plugin folder:

```bash
cp -r omarchy/deezer-player ~/.config/omarchy/plugins/schotime.deezer-player
```

Add **Deezer Player** from the Omarchy bar widget picker. The shell reloads
plugin files automatically. The widget polls `deezer-tui --status --json`,
and sends transport commands through the existing CLI/daemon protocol.

The first version is deliberately a compact native control surface. Search,
library, queue, and login views should be added through a stable JSON IPC
endpoint in the Rust binary rather than duplicating Deezer API logic in QML.
