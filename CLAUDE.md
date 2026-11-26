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
- markdownlint skips `.claude/skills/`, which holds unmodified copies.

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
3. Update this file: the versions, the skills tag, and the notes under "Skills".
   Check that each note is still true of the new copies. Update `README.md` too.
4. Run `mise run gate`, then `cargo run`. Review the `Cargo.lock` diff.
5. In the report, name the old and new versions and say that the skills were updated.

gpui-rsx 0.13.0 lists only GPUI Kit 0.10.0 as verified. This project builds against
0.11.0, but only the syntax used in `src/` has been exercised, so rebuild and run the
app after any version change or new `rsx!` construct.
