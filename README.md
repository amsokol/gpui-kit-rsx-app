# gpui-kit-rsx-app

A minimal desktop app for Linux, macOS and Windows: a label and a button that counts
clicks, in a window that follows the system's look. It uses
[GPUI Kit](https://gpui-kit.com) 0.11.0 and
[gpui-rsx](https://github.com/wsafight/gpui-rsx) 0.13.0. Memory comes from
[mimalloc](https://github.com/amsokol/mimalloc) 3.7.1, built from its C sources, so
the build needs a C compiler.

Only Linux x86_64 (GNOME on Wayland) has been tried by hand. CI is set up to compile and
test the app on Linux, Windows and macOS, each on x86_64 and Arm64. The macOS build
targets macOS 11.0 and later; it has not been run on any macOS by hand. A release build
on Windows opens without a console, so its error messages are not shown.

## The window

- **Title bar.** GPUI Kit's own, laid out like the system's. On macOS and Linux the
  title is centered, bold and has no icon. On Windows it is at the start, in the
  regular weight, after the app's icon. macOS keeps its traffic lights, and Linux
  follows the desktop's button layout. Where the window manager draws a title bar
  itself, the app draws none.
- **Rounded corners.** Where the app decorates its own window on Linux, the corners
  have libadwaita's radius, and those at a maximized or tiled edge are square. While
  the top right corner is rounded, the window buttons keep a little away from the
  right edge. On macOS and Windows the app leaves the corners to the system.
- **Light and dark.** The system's mode at startup, and each change after it.
- **Accent color.** The system's accent color tints the controls, and follows changes.
- **Font.** The system's UI font. On Linux it is read from GNOME's or KDE's setting
  through the XDG settings portal; macOS and Windows get it from GPUI. The size is
  GPUI Kit's, not the system's. For bold text in variable fonts on Linux, Ubuntu's
  and GNOME's among them, the app builds a patched copy of GPUI's text system
  (`vendor/gpui-pre-wgpu.patch`).
- **Size and position.** Size, position, display and the maximized state are saved
  when the window closes or the app quits; a killed process (`Ctrl+C` in the
  terminal) saves nothing. The window returns to its display, no larger than it. If
  that display is gone, the window opens in the center of the primary one. A Wayland
  compositor chooses the position itself, and an X11 window manager may too.

The saved window is in `window.json`:

| System  | Folder                                            |
| ------- | ------------------------------------------------- |
| Linux   | `~/.config/gpui-kit-rsx-app/`                     |
| macOS   | `~/Library/Application Support/gpui-kit-rsx-app/` |
| Windows | `%LOCALAPPDATA%\gpui-kit-rsx-app\`                |

## Run

```bash
cargo run
```

To try another look without changing the system's settings:

| Variable                  | Value                                       |
| ------------------------- | ------------------------------------------- |
| `GPUI_KIT_RSX_APP_THEME`  | `light` or `dark`                           |
| `GPUI_KIT_RSX_APP_ACCENT` | a color such as `#2ecc71`                   |
| `GPUI_KIT_RSX_APP_FONT`   | an installed font family, e.g. `Noto Serif` |

On Debian or Ubuntu, install the native dependencies first (list from the gpui-rsx
demo instructions):

```bash
sudo apt-get install -y libfontconfig1-dev libwayland-dev libx11-dev libxcb1-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libxkbcommon-x11-dev
```

## Check

The checks run through [mise](https://mise.jdx.dev) 2026.11.14 or newer. Rust 1.100
comes from `rust-toolchain.toml` through rustup.

```bash
mise trust             # once per clone, if mise asks
mise install --locked  # the tools of mise.toml, as recorded in mise.lock
mise run gate          # formatting, clippy, dependencies, Markdown, tests
```

CI (`.github/workflows/ci.yml`) runs the gate on Linux x86_64, and clippy and the tests
on the other five platforms: Linux Arm64, and Windows and macOS on x86_64 and Arm64.
