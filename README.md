# gpui-kit-rsx-app

A minimal desktop app: a label and a button that counts clicks. It uses
[GPUI Kit](https://gpui-kit.com) 0.11.0 and
[gpui-rsx](https://github.com/wsafight/gpui-rsx) 0.13.0. The startup code follows
GPUI Kit's `hello_world` example.

## Run

```bash
cargo run
```

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
