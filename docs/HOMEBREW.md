# Homebrew Tap Guide for Harnesscope

Harnesscope is distributed via the official tap [`mr-lexus/homebrew-tap`](https://github.com/mr-lexus/homebrew-tap).

## macOS: install without changing Xcode

```sh
brew update
brew install --cask mr-lexus/tap/harnesscope
harnesscope --version
harnesscope server start
harnesscope ui
```

The binary cask downloads the Apple Silicon or Intel release, verifies SHA-256,
and links the executable. It does not compile locally or require Rust, Node,
Python, or an Xcode/Command Line Tools upgrade. Your developer-tool settings stay
unchanged. Homebrew itself and the binary's macOS runtime requirements still apply.

A formula without a Homebrew bottle can take the source-install path even when
it only copies an upstream binary. That path can report outdated Xcode/CLT.
Explicit `--cask` avoids that build path; `--force` is not a substitute.
Homebrew documents that [casks and bottles do not require developer tools](https://docs.brew.sh/Common-Issues#missing-command-line-tools).
Without the original error, this does not diagnose every possible Homebrew issue.

For updates: `harnesscope server stop`, then
`brew upgrade --cask mr-lexus/tap/harnesscope`, then `harnesscope server start`.
Preserve any custom database-path environment on restart. The cask does not
configure login startup; **`brew services` only manages the formula**. Use the
formula instructions below only when you choose that installation method.

### Switching from an installed formula

Back up existing telemetry first. Do not replace a newer development collector
with an older published binary. Stop the formula service (and any manually started
instance), unlink its binary, then install the cask:

```sh
brew services stop mr-lexus/tap/harnesscope
harnesscope server stop
brew unlink mr-lexus/tap/harnesscope
brew install --cask mr-lexus/tap/harnesscope
harnesscope server start
```

Unlinking preserves the formula and user data. Do not use `--overwrite` to hide
conflicts. To return, stop the collector, uninstall the cask, run
`brew link mr-lexus/tap/harnesscope`, and start the formula service. Do not run
both against the same port/database. The cask does not delete telemetry on uninstall.

### Without Homebrew

The [direct installer](../install.sh) also downloads a checksummed binary into
`~/.local/bin` without local compilation:

```sh
curl -fsSL https://raw.githubusercontent.com/mr-lexus/harnesscope/main/install.sh -o /tmp/harnesscope-install.sh
bash /tmp/harnesscope-install.sh
```

Choose one installation method to avoid PATH/service conflicts. Normal macOS
execution/trust checks still apply; no Gatekeeper or developer settings are disabled.

### Published versions

The checked-in cask/formula reference published **0.2.3**. The retrospective
collector **0.3.0-rc.2** is development code, not yet a downloadable release.
Installing the cask does not install unpublished features; never downgrade a
development database's collector. See [capabilities](CAPABILITIES.md).
Release CI generates both definitions from `checksums.txt` using
`scripts/generate-homebrew.py`; stable releases update both in the tap when its
token is configured. Prereleases do not replace the stable tap.

---

## 1. Formula installation (Linux, or macOS with compatible developer tools)

```bash
# Add tap and install harnesscope
brew install --formula mr-lexus/tap/harnesscope
```

Verify installation:
```bash
harnesscope --version
harnesscope doctor
```

---

## 2. Managing the Background Telemetry Service

Homebrew integrates with system service managers (`launchd` on macOS, `systemd` on Linux):

```bash
# Without sudo, start automatically at user login on macOS
brew services start mr-lexus/tap/harnesscope

# Check service status
brew services list

# Stop background server
brew services stop mr-lexus/tap/harnesscope

# Restart background server
brew services restart mr-lexus/tap/harnesscope
```

When running as a Homebrew service:
- **Port**: default `4242` (http://127.0.0.1:4242)
- **Log file**: `$(brew --prefix)/var/log/harnesscope.log`
- **Error log**: `$(brew --prefix)/var/log/harnesscope.error.log`

---

## 3. Upgrading via Homebrew

```bash
brew update
brew upgrade mr-lexus/tap/harnesscope
brew services restart mr-lexus/tap/harnesscope
```

---

## 4. Manual Server & Web UI (Alternative)

If you prefer not using `brew services`:
```bash
# Open Web UI (starts background daemon automatically if needed)
harnesscope ui

# Or manage server via built-in CLI commands
harnesscope server start
harnesscope server status
harnesscope server stop
```
