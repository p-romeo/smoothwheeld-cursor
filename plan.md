# smoothwheeld — Implementation Plan

## Project summary

**smoothwheeld** is a small Rust daemon for Linux (initially Arch Linux / CachyOS) that reads physical mouse wheel events from evdev devices (`/dev/input/event*`), optionally suppresses the original wheel stream when safe, and emits smoother virtual scroll events through a uinput virtual mouse (`/dev/uinput`). It targets both Wayland and X11 by operating **below** the compositor and display server: no wlroots, Mutter, KWin, or X11-specific injection APIs.

**Design priorities:** correctness, safety, debuggability, and incremental testing—not feature sprawl.

**Important honesty constraint:** True “macOS-like” smooth scrolling cannot be guaranteed for every application. Toolkits interpret `REL_WHEEL`, `REL_WHEEL_HI_RES`, timing, and accumulation differently. The daemon should improve behavior where the input stack allows, without claiming universal perfection.

---

## Architecture

End-to-end pipeline:

```text
physical mouse wheel event
    ↓
evdev reader reads /dev/input/eventX
    ↓
optional device grab suppresses original wheel event
    ↓
scroll smoother converts one wheel tick into smoother output
    ↓
uinput virtual mouse emits REL_WHEEL_HI_RES when supported
    ↓
fallback emits timed REL_WHEEL pulses when needed
    ↓
applications receive smoother scroll input
```

**Assumptions:**

- The compositor or X server will deliver synthetic events from the uinput node like any other kernel input device.
- Initial versions focus only on **scroll wheel axes**; pointer motion and buttons are explicitly out of scope for transformation (see Requirements).

---

## Safety model

Grabbing `/dev/input` devices is high risk: a bug or wrong device can make the pointer or critical input unusable.

**Principles:**

1. **Default: no grab** during early development and for conservative deployments. Duplicate scrolling may occur; that is acceptable until virtual output is verified.
2. **Grab is opt-in** via `--grab` or `grab_device = true`, never the default in config for first-time users (document that the shipped example may still use `false`; product choice: keep default `false` in code and examples).
3. **Never grab keyboard-only or combined keyboard+pointer devices** for “mouse” selection: refuse devices that advertise keyboard capabilities (`EV_KEY` keys in the keyboard range, or use evdev classification helpers—see Phase 2).
4. **Never modify non-wheel events** on the physical device when not grabbing (read-only). When grabbing, only suppress or rewrite wheel-related reports; forward other events unchanged if the architecture merges streams—or only grab for wheel axis injection after validating feasibility (implementation choice: grab entire device but only inject wheel on virtual device; physical device non-wheel events must still reach the user—see Technical Details).
5. **Print selected device path and name before opening for exclusive access** when grab is requested.
6. **Startup delay / confirmation:** optional `--delay-ms` or `--confirm` (plan: optional delay flag for Phase 9) so the user can switch to a terminal with SSH/second keyboard before grab activates.
7. **Kill switch:** foreground mode responds to `Ctrl+C` (SIGINT); document `kill` and systemd stop.
8. **Testing:** recommend testing from a TTY or SSH session with a **second input path** (second mouse, laptop trackpad if different node, or SSH) before enabling grab on the sole pointer.

**systemd:** user service disable/stop must be documented so a bad deployment does not persist (see Phase 10 and Rollback).

---

## Requirements (non-negotiable)

| # | Requirement |
|---|-------------|
| 1 | Implementation language: **Rust**. |
| 2 | Use **evdev** to enumerate and read physical devices. |
| 3 | Use **uinput** to create a virtual mouse device. |
| 4 | Detect candidates by relative wheel capabilities: `REL_WHEEL`, `REL_HWHEEL`, and when present `REL_WHEEL_HI_RES`, `REL_HWHEEL_HI_RES`. |
| 5 | **`--list-devices`**: print candidate devices with name, path, and supported wheel capabilities. |
| 6 | Select device by **`--device` path** or **`--device-name`** substring/glob match (define match semantics in CLI section). |
| 7 | **Do not modify keyboard input** (do not inject or filter key events). |
| 8 | **Do not modify pointer movement** (`REL_X`/`REL_Y` or equivalent) for the app-visible path from this daemon’s virtual device beyond minimal uinput setup if the kernel/API requires defaults—physical movement must not be duplicated or altered by design. |
| 9 | **Do not modify mouse button clicks** (no remapping of `BTN_*`). |
| 10 | **Initial version:** handle **scroll wheel events only** on the physical reader; ignore or pass-through other event types without synthetic button/motion. |
| 11 | Emit **`REL_WHEEL_HI_RES`** (and horizontal hi-res when enabled) on the uinput device when the uinput session and kernel support it. |
| 12 | **Fallback:** multiple timed **`REL_WHEEL` / `REL_HWHEEL`** pulses per physical tick when hi-res is unavailable or disabled. |
| 13 | Clear errors for permission denied on `/dev/input/event*` and `/dev/uinput`. |
| 14 | **Structured logging** (e.g., `tracing` with levels and optional JSON-friendly fields). |
| 15 | **TOML configuration** file. |
| 16 | **systemd** service plan (user and/or system). |
| 17 | **udev** permissions plan. |
| 18 | **Arch `PKGBUILD`** plan. |
| 19 | **Incremental testing strategy** (see Test strategy). |
| 20 | **Rollback / uninstall** documentation. |

---

## CLI design

### Commands and flags

```text
smoothwheeld --list-devices
smoothwheeld --device /dev/input/eventX
smoothwheeld --device-name "Mouse Name"
smoothwheeld --config ~/.config/smoothwheeld/config.toml
smoothwheeld --no-grab
smoothwheeld --grab
smoothwheeld --dry-run
smoothwheeld --verbose
```

### Semantics

| Flag / command | Behavior |
|----------------|----------|
| `--list-devices` | Enumerate `/dev/input/event*`, filter to candidate mice (wheel REL bits, not keyboards), print **path**, **kernel name**, **`REL_*` wheel capabilities** present. Exits 0 after printing. |
| `--device PATH` | Open this evdev node as the physical source. Mutually exclusive with `--device-name` if both given: define precedence (recommended: **CLI error** if both specified). |
| `--device-name SUBSTRING` | Select first matching device by case-sensitive or case-insensitive **substring** of `Device.name()` (document choice: recommend case-insensitive contains). If multiple match, fail with an error listing matches unless `--config` narrows (optional: pick stable order—lowest event index). **Assumption:** exact behavior “first match by sorted path” for determinism. |
| `--config PATH` | Load TOML; merge with CLI (CLI overrides config for overlapping keys). |
| `--no-grab` | Force **exclusive grab off** for the session (overrides config `grab_device = true`). |
| `--grab` | Force **grab on** (overrides `false`). If both `--grab` and `--no-grab`, **error**. |
| `--dry-run` | Open device as needed but **do not create uinput** or **do not write synthetic events** (implementation choice: still open evdev read-only and log decoded wheel events—state explicitly in Phase 3/5). Recommended: **no uinput**, log only. |
| `--verbose` | Shorthand for debug-ish log level (`trace` or `debug` per implementation); may imply `RUST_LOG` if not set. |

**Additional recommended flags (state in implementation):**

- `--delay-ms N` — delay before grab or before main loop (safety).
- `-h` / `--help` — generated help (`clap`).
- `-V` / `--version` — crate version.

**Foreground:** default run mode is **foreground** until Phase 10; no daemonization in early phases.

---

## Config design

**Default path:** `~/.config/smoothwheeld/config.toml`

**Example:**

```toml
device_name_match = ""
device_path = ""
pulse_count = 8
pulse_interval_ms = 12
multiplier = 1.0
invert_scroll = false
enable_horizontal = true
use_high_res = true
fallback_to_wheel_pulses = true
grab_device = false
log_level = "info"
```

| Field | Purpose |
|-------|--------|
| `device_name_match` | Substring to select device when `--device-name` is not passed and CLI does not pin path. Empty means “require explicit CLI or list-devices”. |
| `device_path` | Explicit `/dev/input/eventX` when user wants a stable default without CLI. |
| `pulse_count` | For fallback mode: number of discrete `REL_WHEEL` (or horizontal) pulses per one logical physical tick after smoothing split. |
| `pulse_interval_ms` | Sleep between synthetic pulses (approximate; use monotonic clock). |
| `multiplier` | Scale effective scroll delta (applied in smoother; hi-res and fallback must interpret consistently). |
| `invert_scroll` | Flip vertical (and optionally horizontal if documented) direction. |
| `enable_horizontal` | If false, ignore `REL_HWHEEL*` from source and do not emit horizontal. |
| `use_high_res` | Prefer emitting `REL_WHEEL_HI_RES` / `REL_HWHEEL_HI_RES` on uinput when supported. |
| `fallback_to_wheel_pulses` | If hi-res cannot be used or `use_high_res` is false, use timed `REL_WHEEL` pulses. |
| `grab_device` | Opt-in exclusive grab. **Default false.** |
| `log_level` | `error` / `warn` / `info` / `debug` / `trace` (map to `tracing`). |

**Merge rule:** CLI overrides config. Missing file: use defaults + CLI only; **do not fail** unless user passed `--config` with missing file.

---

## Repository structure

```text
smoothwheeld/
  Cargo.toml
  README.md
  LICENSE
  plan.md
  src/
    main.rs
    cli.rs
    config.rs
    devices.rs
    input.rs
    output.rs
    smoother.rs
    errors.rs
    logging.rs
  packaging/
    arch/
      PKGBUILD
    systemd/
      smoothwheeld.service
    udev/
      99-smoothwheeld.rules
  examples/
    config.toml
  docs/
    architecture.md
    troubleshooting.md
```

| Module | Responsibility |
|--------|----------------|
| `main.rs` | Binary entry: parse CLI, load config, initialize logging, dispatch subcommands/run loop, graceful shutdown. |
| `cli.rs` | `clap` definitions, validation (e.g., `--grab` vs `--no-grab`), default paths. |
| `config.rs` | Serde models, defaults, merge with CLI, path resolution (`dirs`/`directories`). |
| `devices.rs` | Enumerate `/dev/input/event*`, capability checks (`REL_WHEEL`, etc.), filter keyboards, `--list-devices` table, resolve `--device-name`. |
| `input.rs` | Open evdev device, async or blocking read loop, extract wheel events only, optional grab. |
| `output.rs` | Open uinput virtual mouse, set ABS not used, REL wheel keys, sync events, capability probe for hi-res. |
| `smoother.rs` | Map physical ticks → output segments (hi-res deltas or pulse schedule), `multiplier`, `invert`, horizontal toggle. |
| `errors.rs` | Typed errors for permissions, EVIOCGRAB failures, uinput creation, IO. |
| `logging.rs` | `tracing` subscriber setup, `--verbose`, `log_level` from config. |

---

## Dependency guidance (verify before coding)

Likely crates:

- **`evdev`** — Linux evdev access; device listing, capabilities, reads, grab.
- **uinput:** `uinput`, `input-linux`, or **`input-linux`** / community-maintained wrappers — **the implementation agent must read current docs and MSRV**; uinput API surface varies.
- **`clap`** — CLI.
- **`serde`** + **`toml`** — config.
- **`thiserror`** — library-style error enums at boundaries; **`anyhow`** optional for top-level binary glue if desired.
- **`tracing`** + **`tracing-subscriber`** — structured logging (filters, fmt).
- **`dirs`** or **`directories`** — XDG config dir resolution.

**Mandatory note:** Before writing code, verify crate APIs, feature flags, and Linux minimum kernel assumptions against current documentation and repository.

---

## Technical details

### Identifying candidate mouse devices

- Enumerate `glob("/dev/input/event*")` or read `/dev/input` (order by path).
- Open each with read-only evdev where possible; read `device.supported_relative()` (or equivalent) for `REL_WHEEL`, `REL_HWHEEL`, `REL_WHEEL_HI_RES`, `REL_HWHEEL_HI_RES`.
- Exclude devices with **keyboard**: e.g., if `KEY_A` … or `KEY_MIN_INTERESTING` keyboard set is supported, or use `evdev`’s key classification if available—or conservative rule: if `EV_KEY` supports typical letter keys, exclude. **Assumption:** at minimum exclude if full keyboard QWERTY range present; refine using `libinput` heuristics docs if needed.

### Avoiding keyboard devices

- Prefer explicit capability-based exclusion over name matching.
- Never use `/dev/input/by-id/...-kbd` style nodes if name-based selection is ambiguous; path selection is safest.

### Horizontal scrolling

- If `enable_horizontal` and source emits `REL_HWHEEL` / `REL_HWHEEL_HI_RES`, apply same smoother pipeline to horizontal axis independently of vertical.

### High-resolution wheel events

- Physical devices may already emit hi-res. **Smoother** should consume either discrete steps or hi-res accumulation and produce a consistent internal “delta per physical notch” model where possible.
- uinput virtual device must advertise `REL_WHEEL_HI_RES` in relative bits if the create API allows; sync and emit `EV_REL` events with hi-res values per kernel convention (verify units: often multiples matching `wheel click`).

### Duplicate scrolling without grab

- Physical device continues to deliver events to the compositor; the virtual device **also** emits scroll → **double scrolling**.
- **Why no-grab is still useful:** validates smoothing and uinput path without risking exclusive grab on the only pointer.

### Grab mode behavior

- **Exclusive grab** (`EVIOCGRAB` or equivalent) stops other clients from receiving events from **that** physical node—but the compositor usually reads the same node: user may see **only** smoothed scroll from virtual device if grab works and compositor no longer gets physical wheel from that fd. **Caveat:** multi-seat and libinput quirks—**test per machine**.
- **Safety:** wrong device grab can remove pointer; hence keyboard refusal and preflight prints.

### Why application behavior varies

- GTK/Qt/terminals map wheel differently; some coalesce; some ignore hi-res; browsers have their own smooth-scroll logic.

### Foreground-first

- **Stderr log visibility**, easy `Ctrl+C`, strace/debug without systemd journal noise in early phases.

### Permission failures

- On `EACCES` / `EPERM` for `/dev/input/*` or `/dev/uinput`: print actionable message: add user to `input` group, udev rules, or temporary `root`, and note **logout/login after `usermod`**.

### Inspecting devices

- `libinput list-devices`, `libinput debug-events` (Wayland/X generally), `evtest` if installed, `udevadm info /dev/input/eventX`.

### Stopping the daemon

- `Ctrl+C` in foreground; `systemctl --user stop smoothwheeld` when installed as user service; `sudo kill PID` as last resort.

---

## Implementation phases

Each phase lists: **Goal**, **Files touched**, **Tasks**, **Expected behavior**, **Manual tests**, **Failure cases**, **Exit criteria**.

---

### Phase 1: Repository and Rust CLI skeleton

- **Goal:** Runnable binary, `--help`, logging init, no device I/O required.
- **Files:** `Cargo.toml`, `src/main.rs`, `src/cli.rs`, `src/logging.rs`, `src/errors.rs` (minimal).
- **Tasks:**
  - `cargo new` style project layout; add `clap`, `tracing`, `tracing-subscriber`.
  - Implement stub subcommands for `--list-devices` (may print “not implemented” until Phase 2, or hide behind feature—prefer implementing real `--list-devices` in Phase 2 only; Phase 1 can accept flag and exit 0 with placeholder **only if** README clarifies; **better:** wire flags and print help success).
  - Initialize `tracing` from `--verbose` / default info.
- **Expected behavior:** `cargo run -- --help` shows all planned flags; no panic.
- **Manual tests:** `cargo run -- --help`, `cargo run -- --verbose`.
- **Failure cases:** missing features in `clap` parse.
- **Exit criteria:** Clean compile; help text documents later behavior.

---

### Phase 2: Device enumeration

- **Goal:** `--list-devices` fully functional.
- **Files:** `src/devices.rs`, `src/main.rs`, `src/cli.rs`.
- **Tasks:**
  - Enumerate event nodes; open evdev; query REL capabilities; exclude keyboards.
  - Pretty-print table: `path`, `name`, `REL_WHEEL` / `REL_HWHEEL` / hi-res flags.
- **Expected behavior:** List matches machine mice; no keyboard-only boards listed.
- **Manual tests:** `cargo run -- --list-devices`; compare with `libinput list-devices`.
- **Failure cases:** permission errors listed per device; non-fatal.
- **Exit criteria:** Operator can identify correct `eventX` for their mouse.

---

### Phase 3: Physical wheel event reader

- **Goal:** Read and log wheel events only; no uinput.
- **Files:** `src/input.rs`, `src/main.rs`.
- **Tasks:**
  - Open `--device` path; loop read `InputEvent`s; filter `EV_REL` for wheel axes; log direction and value.
  - Do not grab.
- **Expected behavior:** Rolling wheel produces structured log lines.
- **Manual tests:** `cargo run -- --device /dev/input/eventX --dry-run` (if dry-run implies read-only—align with Phase 5).
- **Failure cases:** `EACCES`, wrong device (no wheel), unplug.
- **Exit criteria:** Dry-run confirms physical path.

---

### Phase 4: Virtual uinput mouse output

- **Goal:** Create uinput virtual mouse; emit **test** scroll without physical reader.
- **Files:** `src/output.rs`, `src/main.rs`, optional `src/cli.rs` flag `--uinput-self-test` (or subcommand).
- **Tasks:**
  - Create virtual device with REL bits including hi-res if supported.
  - Emit a short sequence: hi-res vertical scroll or timed pulses (configurable test).
- **Expected behavior:** In `libinput debug-events`, synthetic events appear on a new device node while test runs.
- **Manual tests:** Run self-test; observe with `libinput debug-events` or `evtest` on the new device.
- **Failure cases:** `/dev/uinput` permissions; creation EINVAL from bad bitmask.
- **Exit criteria:** Operator sees virtual wheel without physical smoothing.

---

### Phase 5: Basic pass-through smoothing loop (no grab)

- **Goal:** Connect Phase 3 reader to Phase 4 output; map each physical wheel event to smoother output; **no grab**.
- **Files:** `src/smoother.rs`, `src/main.rs`, `src/input.rs`, `src/output.rs`.
- **Tasks:**
  - Define internal representation of “one user notch” vs raw values.
  - Initial smoothing: e.g., split one notch into N subsamples OR scaled hi-res burst (simple algorithm—document choice).
- **Expected behavior:** Virtual scroll feels smoother; **duplicate** scroll likely.
- **Manual tests:** `cargo run -- --device /path` (no dry-run); scroll in editor/browser; expect doubled scroll without grab.
- **Failure cases:** event merge ordering, SYN_DROPPED (handle or log).
- **Exit criteria:** Usable improvement observable on at least one app when ignoring duplicate.

---

### Phase 6: High-resolution wheel support

- **Goal:** Prefer `REL_WHEEL_HI_RES` / `REL_HWHEEL_HI_RES` on uinput when available; read hi-res from source intelligently.
- **Files:** `src/output.rs`, `src/smoother.rs`, `src/input.rs`.
- **Tasks:**
  - Probe output caps; if hi-res supported, emit it; else branch to Phase 7 paths.
  - Tune scaling so one physical click ≈ expected app delta (document tunables).
- **Expected behavior:** Hi-res path produces fine-grained deltas in debug tools.
- **Manual tests:** `libinput debug-events` shows hi-res on virtual device.
- **Failure cases:** kernel or crate lacks hi-res uinput support—fall back.
- **Exit criteria:** `use_high_res = true` path works on dev machine or explicit fallback taken with log.

---

### Phase 7: Timed pulse fallback

- **Goal:** `fallback_to_wheel_pulses`: one tick → `pulse_count` × `REL_WHEEL` with `pulse_interval_ms`.
- **Files:** `src/smoother.rs`, `src/output.rs`, `config.rs` integration later.
- **Tasks:**
  - Schedule pulses without blocking the whole world: sleep in small chunks or use async timer—acceptable to block thread in MVP if documented.
- **Expected behavior:** Apps that only see discrete steps still get multiple smaller steps.
- **Manual tests:** Force `use_high_res = false` and `fallback_to_wheel_pulses = true`.
- **Failure cases:** backlog if user scrolls fast—implement queue cap or coalesce with logs.
- **Exit criteria:** No panic under rapid scroll; behavior degrades gracefully.

---

### Phase 8: TOML config

- **Goal:** Load `~/.config/smoothwheeld/config.toml`; merge CLI overrides.
- **Files:** `src/config.rs`, `examples/config.toml`, `src/main.rs`.
- **Tasks:**
  - Serde defaults; validate ranges (positive pulse count, non-negative interval, reasonable multiplier cap optional).
  - Document merge order.
- **Expected behavior:** Same as Phase 5–7 but configurable without rebuilding.
- **Manual tests:** Edit example config; run with/without `--config`.
- **Failure cases:** Malformed TOML → error with path and line if possible.
- **Exit criteria:** All config keys read and applied.

---

### Phase 9: Safe device grab mode

- **Goal:** Optional `EVIOCGRAB` / exclusive capture: `--grab`, `grab_DEVICE` config, overridden by `--no-grab`.
- **Files:** `src/input.rs`, `src/cli.rs`, `src/config.rs`, `docs/troubleshooting.md`.
- **Tasks:**
  - Pre-flight: print device name, path, capabilities; refuse keyboards; refuse grab if sanity checks fail.
  - Optional `--delay-ms` before grab.
  - Ensure **pointer motion and buttons** from physical device still reach userspace: **critical** — if whole-device grab is used, **forward** non-wheel events by injecting into a second virtual device **or** document that grab mode requires a second physical pointer path. **Assumption for this plan:** simplest MVP grab forwards all non-wheel events unchanged via reading evdev and writing duplicate to uinput for REL/KEY—but duplicating relative motion breaks (doubles movement). **Therefore:** grab mode **must** either (a) only be supported when the stack allows fine-grained suppression (not generally available), or (b) use kernel **`EVIOCGRAB`** knowing it blocks all clients from physical device—including compositor—so **the compositor loses the device entirely** unless smoothwheeld forwards everything. Forwarding **all** events through a virtual device is non-trivial (requires creating a full proxy). **Pragmatic MVP:** Document that **grab mode is “experimental”** and may require **only wheel** suppression via not grabbing entire device—actually impossible without grab. **Engineering decision:** Phase 9 implements **exclusive grab** and documents that user must rely on **virtual uinput device for all pointer events** only if we implement full proxy (**out of scope**). **Revised pragmatic approach:** **Do not ship full grab** in MVP if it would freeze pointer; instead, implement **grab** only after **libevdev**-style re-routing exists—or ship grab as **warnings** and **best-effort**: exclusive grab + **read loop forwards REL_XY and BTN via uinput** to reproduce pointer (**complex**).

**Plan decision for implementers:** Phase 9 should implement **grab** only if the Phase 5 architecture already opens both physical read and injects **only scroll** on virtual while compositor still reads physical for motion—or **use `libei`/`udevmon`**—**reject**: out of scope.

**Concrete acceptable MVP for grab:** Use **exclusive grab** on the physical mouse **briefly marked unsupported** if full pointer break occurs—or: grab and **forward** motion/button events through uinput with **relative** injection (**must** match kernel timing). This is hard. **Chosen MVP:** Implement **`grab` as best-effort**: call grab; **warn** user that **pointer may not move** if compositor depended on same fd; recommend testing with **second device**. Exit criteria: grab suppresses duplicate scroll **when compositor still receives motion from same device** — **impossible with exclusive grab**. **Final plan statement:** Phase 9 documents this conflict: **exclusive grab hides all events from compositor**; therefore smoothwheeld **must forward REL_X, REL_Y, BTN_** from grabbed device to uinput virtual device in the same order and timing. **Tasks:** implement full event proxy for **non-wheel** events unchanged; **wheel** events replaced by smoothed virtual stream on **virtual device only** and **not** re-injected as raw physical wheel from grab path—suppress physical wheel by consuming those REL events and not forwarding them; forward other REL/KEY unchanged. **Files:** extend `input.rs` loop: on grab, write to uinput motion/buttons as read, wheel events diverted to smoother then to uinput scroll only.

- **Expected behavior:** With grab, no duplicate wheel; motion/buttons behave as before via forwarding.
- **Manual tests:** Second mouse available; enable grab; verify no double scroll and pointer still works.
- **Failure cases:** if forwarding bug, **.pointer stuck** — exit criteria includes **manual verification** of motion.
- **Exit criteria:** Grab mode safe enough for testers; document “need second mouse if broken”.

---

### Phase 10: systemd and udev integration

- **Goal:** User (or system) service + udev rules for permissions.
- **Files:** `packaging/systemd/smoothwheeld.service`, `packaging/udev/99-smoothwheeld.rules`, `README.md`.
- **Tasks:**
  - **User service** recommended: `After=graphical-session.target`; `Environment` if needed; document **device path** configuration when session lacks plugdev.
  - udev: `TAG+="uaccess"` or group `input` — follow distro patterns; reload rules.
- **Tradeoffs:** User service has user session context; system service runs as root (avoid if possible).
- **Expected behavior:** `systemctl --user enable --now smoothwheeld.service` starts daemon.
- **Manual tests:** enable/disable; journal logs.
- **Failure cases:** wrong `WantedBy` target; permission still denied.
- **Exit criteria:** Repeatable install on Arch template VM.

---

### Phase 11: Arch packaging

- **Goal:** `PKGBUILD` installs binary, systemd unit, udev, example config, docs.
- **Files:** `packaging/arch/PKGBUILD`.
- **Tasks:**
  - Standard `archlinux` packaging: depends `glibc`, optional `systemd`; build from tagged source.
  - Install to `/usr/bin/smoothwheeld`, `/usr/lib/systemd/user/smoothwheeld.service`, `/usr/lib/udev/rules.d/99-smoothwheeld.rules`, `/usr/share/doc/...`.
- **Expected behavior:** `pacman -Ql` lists files; package removes cleanly.
- **Manual tests:** `makepkg -si` in VM.
- **Failure cases:** FHS paths wrong.
- **Exit criteria:** Package review-ready.

---

### Phase 12: Testing, troubleshooting, and documentation

- **Goal:** `README.md`, `docs/architecture.md`, `docs/troubleshooting.md`, known limitations, MVP checklist.
- **Files:** `docs/*`, `README.md`.
- **Tasks:**
  - Document duplicate-scroll expectation without grab.
  - Document grab + forwarding caveat.
  - Include `libinput` commands, permission fixes, rollback steps.
- **Expected behavior:** New user can follow without author.
- **Manual tests:** Run through troubleshooting flow with denied permissions.
- **Failure cases:** doc drift from CLI.
- **Exit criteria:** Definition of Done met.

---

## Test strategy (incremental)

1. **Unit-ish:** pure functions in `smoother.rs` (delta in → schedule out) with `#[cfg(test)]`.
2. **Integration:** manual Linux host; no CI assumption for real evdev in GitHub unless `virtme` or kvm added later—**optional future**.
3. **Order:** Phase 2 list → Phase 3 dry-run → Phase 4 self-test → Phase 5 combined → Phase 6–7 edge → Phase 9 only with spare input.
4. **Recording:** encourage `libinput debug-events` transcript in bug reports (redact serials).

---

## Packaging plan summary

- **Source layout:** under `packaging/` as above.
- **Binary:** `/usr/bin/smoothwheeld`.
- **systemd:** user unit in `/usr/lib/systemd/user/` (or `lib` vs `usr/lib` per Arch packaging standards—follow Arch guidelines at package time).
- **udev:** `99-smoothwheeld.rules` documenting `uinput` and `input` group synergies.

---

## Troubleshooting (quick reference)

| Symptom | Checks |
|---------|--------|
| Permission denied | `groups`; `ls -l /dev/uinput`; udev rules; **re-login** after group add. |
| No virtual device | kernel modules `uinput` loaded; `modprobe uinput`. |
| Double scroll | expected without grab; enable grab only after testing. |
| Pointer stuck with grab | stop service; Phase 9 forwarding bug—use spare mouse or SSH. |
| No hi-res | fallback pulses; verify kernel/crate support. |

---

## Commands reference

```bash
cargo new smoothwheeld
cargo run -- --help
cargo run -- --list-devices
cargo run -- --device /dev/input/eventX --dry-run
sudo usermod -aG input "$USER"
# After group change: log out and back in (or reboot); document prominently
ls -l /dev/uinput
ls -l /dev/input/event*
libinput list-devices
libinput debug-events
systemctl --user enable --now smoothwheeld.service
systemctl --user disable --now smoothwheeld.service
sudo systemctl disable --now smoothwheeld.service
```

**Warning:** `usermod -aG input` requires **logout/login** to apply supplementary groups in most sessions.

---

## Rollback / uninstall

1. **Foreground/dev:** `Ctrl+C`.
2. **User service:** `systemctl --user disable --now smoothwheeld.service`; optionally `rm ~/.config/systemd/user/default.target.wants/smoothwheeld.service` if enabled.
3. **Package:** `sudo pacman -Rns smoothwheeld` (remove config in home manually).
4. **udev:** remove rule file and `sudo udevadm control --reload-rules && sudo udevadm trigger`.
5. **Groups:** user may remain in `input` (harmless); remove only if desired (`gpasswd`).

---

## Known limitations

- Not all applications honor high-resolution wheel events.
- Some apps treat wheel input as discrete steps only.
- Wayland compositors own input routing at a higher level; this project deliberately avoids compositor APIs and works through kernel devices—**permission and policy constraints apply**.
- evdev/uinput requires appropriate permissions; this is not a “zero-setup” cross-user solution.
- Grabbing or mis-identifying devices can disrupt input—strict checks and documentation are mandatory.
- Perfect macOS-like scrolling is **not** guaranteed.

---

## Future enhancements (out of scope for initial plan)

- Per-application profiles, GUI/tray, dynamic device hotplug switching, **libinput** filter-like integration, machine-learned scroll curves, Flatpak portal integration (generally incompatible with this low-level approach).

---

## MVP definition (first success)

1. `smoothwheeld --list-devices` shows candidate mice with wheel caps.
2. `smoothwheeld --device /dev/input/eventX --dry-run` logs physical wheel events.
3. A dedicated uinput self-test emits virtual scroll visible in `libinput debug-events`.
4. Without grab: smoothed virtual scroll works; **duplicate** scrolling may occur.
5. With grab (and forwarding per Phase 9): original wheel suppressed from compositor path as designed, smoothed events from virtual device, pointer/button path preserved via forwarding **or** documented limitation if not implemented—**implementation must satisfy forward-or-disable rule before claiming (5)**.

---

## Definition of done (project)

- Phases 1–12 complete; README and docs accurate; `PKGBUILD` builds; user service starts; rollback doc validated once; no panics on unplug (graceful error and exit or retry); tests for `smoother` logic; logging clear on permission failures.

---

## Assumptions log (for implementers)

- Config and CLI defaults favor **safety over convenience** (`grab_device = false`).
- `--device-name` match uses **deterministic** ordering (sorted path) when ambiguous.
- Phase 9 requires **event forwarding** for non-wheel if exclusive grab is used; otherwise grab feature must be disabled with explicit error.

This plan is intentionally specific so an agent can implement phase-by-phase without further product questions; implementation agents must still **verify external crate APIs** against current documentation at implementation time.
