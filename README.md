# smoothwheeld

Linux daemon (Rust) that reads mouse wheel events from [evdev](https://www.freedesktop.org/software/libevdev/doc/latest/) and (eventually) emits smoother scrolling via a [uinput](https://www.kernel.org/doc/html/latest/input/uinput.html) virtual device—below Wayland/X11, without compositor-specific APIs.

See **`plan.md`** for architecture, safety model, phased implementation, and CLI/config contracts.

## Status

Early development: **`--list-devices`** and **`--dry-run`** (wheel event logging) work; full smoothing and uinput output are not implemented yet (see phases in `plan.md`).

## Build

Requires a recent Rust toolchain (dependencies may need **Rust 1.85+**; use `rustup update` if `cargo` fails to parse a dependency crate).

```bash
cargo build --release
```

## Quick usage

```bash
cargo run -- --help
cargo run -- --list-devices
cargo run -- --device /dev/input/event6 --dry-run
```

`--dry-run` opens the device and logs wheel-related `REL_*` events until you press Ctrl+C. It does not create a uinput device.

## Permissions

Reading `/dev/input/event*` usually requires membership in the `input` group or running with elevated privileges. Changing groups requires logging out and back in.

## License

MIT. See `LICENSE`.
