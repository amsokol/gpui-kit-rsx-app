# gpui-kit-rsx-app

Desktop app built on GPUI Kit 0.11.0, with views written in `rsx!` from gpui-rsx 0.13.0.

## Commands

```bash
cargo run
mise run gate     # before a commit: fmt, clippy, cargo-deny, cargo-shear, markdownlint, tests
mise run format   # format the Rust in place
```

`mise.toml` holds the tools and tasks; Rust comes from `rust-toolchain.toml`.

- Fix what the gate reports. Don't silence a check to make it pass.
- An exception in `deny.toml` (an ignored advisory or another license) needs a comment
  with the reason; ask the owner before adding one.
- markdownlint skips `.claude/skills/`, which holds unmodified copies, and `vendor/`.

## Patched dependency

`vendor/gpui-pre-wgpu/` is `gpui-pre-wgpu` 0.3.14, GPUI's text system and renderer on
Linux, with one fix, and `[patch.crates-io]` in `Cargo.toml` builds it. Unpatched, it
uses every variable font at its default weight, so the UI font of GNOME and Ubuntu has
no bold. `tests/font_weights.rs` fails without the fix.

- `vendor/gpui-pre-wgpu.patch` is the whole change to the registry's package, which
  lacks only its `Cargo.lock` here. Edit nothing else there. After a change to the
  code, regenerate the patch; keep the notice at the top of the changed file (Apache-2.0).
- The copy is not a workspace member, so the gate doesn't format, lint or test it. It
  is no longer a registry crate either, so don't count on `cargo-deny` to report an
  advisory for it. Its own tests don't build: the published package lacks the fonts
  they include.
- Keep our project rules (lints, no panics) for our code; don't restyle upstream code.

## Platforms

The app targets Linux, macOS and Windows, each on x86_64 and Arm64, but only Linux
x86_64 can be run here.

- Leave platform work to GPUI Kit and the libraries where they do it: the window
  buttons and dragging (`TitleBar`), the accent color (`mundy`), the config folder
  (`dirs`).
- Write a branch on the system as data: a function of `platform::Os`, or `cfg!()`.
  Then every branch compiles and is linted here, and a test can cover it. Use `#[cfg]`
  only for code that can't compile elsewhere. Today that is the `portal` module in
  `src/system_font.rs`, which has a stub for the other systems; keep both in step.
- macOS and Windows always report `Decorations::Server` (GPUI's default), although
  the app draws the title bar there. Read decorations only through
  `platform::draws_own_title_bar`.
- `GPUI_KIT_RSX_APP_THEME`, `_ACCENT` and `_FONT` force a look (`README.md`).
- `.github/workflows/ci.yml` is the only check of the five other platforms. Don't claim
  that something works there beyond what it ran. Pin each action to the commit of a
  release at least 2 days old.
- The platforms are listed twice: the CI jobs, and `[graph].targets` in `deny.toml`.
  Change them together.

## Allocator

`src/main.rs` makes `mimalloc` the global allocator. It is the owner's wrapper,
<https://github.com/amsokol/mimalloc>, with Microsoft's C sources as a submodule.

- Take it from git by the tag of a release, never from crates.io: the `mimalloc` there
  is another project.
- `deny.toml` (`[sources].allow-git`) allows this one git repository. Ask the owner
  before allowing another.
- To update, change `tag` in `Cargo.toml` and check that `Cargo.lock` moves to the
  commit of that tag. Don't run a plain `cargo update` for it alone.

## Lints

`Cargo.toml` (`[lints]`) and `clippy.toml` hold the rules: `clippy::pedantic` (except
`wildcard_imports`), no `unsafe` code, no unreachable `pub`, no `let _ =` on a
`#[must_use]` value, no `dbg!` or `println!` outside tests, and no panics (below).

- Fix the code rather than silence a lint. An `#[allow]` or `#[expect]` needs
  `reason = "…"`; ask the owner before adding one to production code.
- Don't remove or weaken a lint to make the gate pass.

## No panics in production code

Clippy denies panicking macros and calls (`[lints.clippy]` in `Cargo.toml`, settings in
`clippy.toml`): `panic!`, `todo!`, `unimplemented!`, `unreachable!`, the `assert!` and
`debug_assert!` families, `unwrap`, `expect`, indexing and string slicing. Return a
`Result` or handle the case. The skills' examples use `.expect(…)`; don't copy that.

- Never add `#[allow]` or `#[expect]` for these lints in production code. If one looks
  unavoidable, or a dependency's macro triggers `disallowed_macros`, ask the owner.
- Tests may panic. Each crate root (`src/main.rs`, a future `src/lib.rs`) has a
  `cfg_attr(test, allow(…))` for this. A file in `tests/` needs the same lints in an
  `#![allow(…)]` at its top.
- The lints don't catch every panic: integer overflow, division by zero, and panics
  inside dependencies remain possible.
- In a test module, import names one by one, not with `use super::*`. That glob also
  brings in `gpui::*`, whose `test` macro shadows `#[test]`, and the build fails with
  "recursion limit reached while expanding `#[test]`".

## Skills

For any GPUI work, use the skills in `.claude/skills/`: `gpui-kit` (setup, components,
GPUI mechanics, Coding Guides) and `gpui-kit-design-guides` (read before changing UI).
They are unmodified copies from the `v0.11.0` tag of
<https://github.com/longbridge/gpui-kit> (Apache-2.0). Don't edit them by hand.

How they apply here:

- The skills write `gpui_kit::`. This project aliases the crate as `gpui`, so write
  `gpui::` instead (`gpui_kit::component::…` becomes `gpui::component::…`). The alias
  is required because `rsx!` expansions and GPUI derives emit `gpui::` paths.
- The skills show builder chains. Write views with `rsx!`; the same ownership, identity
  and design rules apply. See the [gpui-rsx docs](https://wsafight.github.io/gpui-rsx/).
- In an `rsx!` class, a spacing number means pixels: `gap-2` is 2 px, not GPUI's
  `gap_2()` (8 px, which follows the base font as the design guides ask). For the rem
  scale, call the method as an attribute: `<div class="flex" gap_2>`. Nothing fails
  when this is wrong, so look at the window.
- `references/usage.md` shows `gpui-kit = "0.7"`, which is outdated upstream.
  `Cargo.toml` is the source of truth for versions.

## Dependency quarantine

`.cargo/config.toml` stops Cargo (1.100+) from resolving a crate version published
less than 2 days ago.

- Never bypass it, even though Cargo's error message suggests
  `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`. Don't set that variable or
  `CARGO_REGISTRY_GLOBAL_MIN_PUBLISH_AGE`, and don't loosen the config. If a version
  is too new, take an older one or wait.
- Versions already in `Cargo.lock` are not rechecked, and a version with no publish
  time in the registry passes unchecked. Review `git diff Cargo.lock` after any
  command that may resolve dependencies.
- mise's `minimum_release_age` covers what mise resolves itself, such as the npm
  packages behind markdownlint, but not the exact versions in `mise.toml`. Check their
  publish time before setting or moving one, then run `mise lock` and commit
  `mise.lock` and `.mise/locks/`.
- Neither setting covers what is fetched by other means, such as the skills.
  Check the age by hand: at least 2 days.
- `mimalloc` is the owner's own crate and is exempt: take a new version at once.

## Updating dependencies

**When the `gpui-kit` version changes, update its skills in the same change.** This
applies to any request to update dependencies, even one that doesn't mention the
skills. The skills must always come from the tag that matches the version in
`Cargo.toml`, never from `main`.

1. Pick the newest `gpui-kit` release that has passed the quarantine, and set it in
   `Cargo.toml` as an exact version (`=X.Y.Z`). Check whether a newer `gpui-rsx` fits it.
2. Replace both skill folders whole, so that files removed upstream don't linger.
   Clone the tag into a new temporary directory (`$tmp`):

   ```bash
   git clone --depth 1 --branch vX.Y.Z https://github.com/longbridge/gpui-kit "$tmp"
   rm -rf .claude/skills/gpui-kit .claude/skills/gpui-kit-design-guides
   cp -r "$tmp/skills/gpui-kit" "$tmp/skills/gpui-kit-design-guides" .claude/skills/
   ```

   If `$tmp/skills/` has other skills, or one of these two is gone, tell the owner.
3. Carry the patch over (see "Patched dependency"). Check whether the new
   `gpui-pre-wgpu` still loads fonts at one weight: remove `[patch.crates-io]` and run
   `cargo test --test font_weights`. If it passes, delete `vendor/`. If it fails,
   replace the copy with the new version, apply the patch, restore `[patch.crates-io]`,
   and run the test again. Cargo only warns that a patch "was not used" when the
   versions no longer match, so look for that warning: the fix is silently gone then.
4. Update this file: the versions, the skills tag, and the notes under "Skills".
   Check that each note is still true of the new copies. Update `README.md` too.
5. Run `mise run gate`, then `cargo run`. Review the `Cargo.lock` diff.
6. In the report, name the old and new versions and say that the skills were updated.

gpui-rsx 0.13.0 lists only GPUI Kit 0.10.0 as verified. This project builds against
0.11.0, but only the syntax used in `src/` has been exercised, so rebuild and run the
app after any version change or new `rsx!` construct.
