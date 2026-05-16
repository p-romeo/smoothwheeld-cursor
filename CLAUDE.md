# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build --release
cargo run -- --help
cargo run -- --list-devices
cargo run -- --device /dev/input/eventX --dry-run
cargo test
cargo clippy
```

Running a single test:
```bash
cargo test axis_label_maps_wheel_variants
```

## Architecture

`plan.md` is the authoritative source of truth for this project. **Read it before implementing new features, changing CLI/config, or touching device interaction behavior.** Do not revise `plan.md` unless the user explicitly asks.

### Pipeline

```
/dev/input/eventX (evdev)
    → input.rs (read loop, filter EV_REL wheel axes)
    → smoother.rs (map tick → hi-res delta or pulse schedule)
    → output.rs (emit via uinput virtual device)
```

### Module responsibilities

| Module | Role |
|--------|------|
| `cli.rs` | `clap` structs + validation (e.g. `--grab`/`--no-grab` conflict) |
| `config.rs` | TOML model, defaults, CLI-overrides-config merge; default path `~/.config/smoothwheeld/config.toml` |
| `devices.rs` | Enumerate `/dev/input/event*`, capability checks, keyboard exclusion, `--list-devices` table, `--device-name` resolution |
| `input.rs` | Open evdev device, blocking read loop, optional exclusive grab (`EVIOCGRAB`) |
| `output.rs` | Create uinput virtual mouse, emit `REL_WHEEL_HI_RES` (preferred) or timed `REL_WHEEL` pulses |
| `smoother.rs` | Pure transformation: physical tick → output segments; `multiplier`, `invert_scroll`, horizontal toggle |
| `errors.rs` | Typed errors for permissions, grab failures, uinput creation |
| `logging.rs` | `tracing` subscriber init; `--verbose` / `log_level` from config |

### Current status (Phases 1–3 complete)

- `--list-devices` and `--dry-run` (wheel event logging) work.
- `smoother.rs` and `output.rs` (uinput) are stubs — Phase 4+ not yet implemented.
- The main run loop (`run_smoothing`) returns an error until Phase 4–5 are done.

### Key design constraints

- **No keyboard events modified.** Devices are excluded if they advertise keyboard keys (`KEY_SPACE` + `KEY_A` heuristic in `devices.rs`).
- **No pointer motion or button events modified.**
- **Grab is opt-in** (`--grab` / `grab_device = true`); default is `false`. Exclusive grab requires forwarding all non-wheel events through uinput (see Phase 9 in `plan.md` for the full complexity).
- **`--device-name`** matches case-insensitively, sorts by path for determinism, errors on 0 or >1 match.
- CLI overrides config for all overlapping keys; missing config file is non-fatal unless `--config` was explicitly passed.

### Permissions

Reading `/dev/input/event*` requires membership in the `input` group or elevated privileges. `/dev/uinput` requires the `uinput` module loaded and appropriate permissions. Group changes require logout/login.
