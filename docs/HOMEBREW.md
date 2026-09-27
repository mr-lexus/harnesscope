# Homebrew Tap Guide for Harnesscope

Harnesscope is distributed via the official tap [`mr-lexus/homebrew-tap`](https://github.com/mr-lexus/homebrew-tap).

---

## 1. Quick Installation

```bash
# Add tap and install harnesscope
brew install mr-lexus/tap/harnesscope
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
# Start background server automatically on system boot
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
