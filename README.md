# MacMousePaste

MacMousePaste is a lightweight macOS menu bar utility that brings the classic Linux/X11 **select-to-copy and middle-click-to-paste** workflow to macOS.

It runs entirely in the background as a menu bar app and lets you paste selected text anywhere using the middle mouse button — without overwriting your normal clipboard.

---

# ✨ Idea and Motivation

On Linux (X11), there are two buffers:

- **Primary Selection** → automatically updated when you select text
- **Clipboard** → updated when you press Ctrl+C

Middle-click pastes the Primary Selection.

macOS does not have this feature natively.

MacMousePaste simulates this behavior by:

- detecting text selection
- saving it internally
- pasting it on middle click
- preserving your real clipboard

This gives macOS the same fast workflow Linux users rely on.

---

# 🧠 How it works

MacMousePaste uses macOS Quartz Event Tap to monitor mouse events.

## Selection flow

When you select text (drag or double click):

1. Simulates Cmd+C
2. Reads clipboard using `pbpaste`
3. Restores original clipboard using `pbcopy`
4. Saves selected text internally

## Paste flow

When you middle-click:

1. Focus click at cursor
2. Temporarily writes stored text to clipboard
3. Simulates Cmd+V
4. Restores original clipboard

Result:

- Primary selection paste works
- Your clipboard remains unchanged

---

# 🖥️ App behavior

MacMousePaste runs as a **menu bar app**:

Menu contains:

- Status: Running / Stopped
- Start / Stop toggle
- Quit

Menu bar icon indicates state:

- 📋✅ Running
- 📋⏸ Stopped

No Dock icon appears.

---

# 📦 Build Instructions

Requirements:

- macOS
- Rust toolchain installed

Install Rust if needed:

```bash
curl https://sh.rustup.rs -sSf | sh
```

Build the app bundle:

```bash
./scripts/build_app.sh
```

Output:

```
target/release/MacMousePaste.app
```

---

# 📥 Install Instructions

Install into Applications and enable autorun:

```bash
./scripts/install.sh
```

This will:

- install to

```
~/Applications/MacMousePaste.app
```

- enable Start at Login
- start the app automatically

---

# ▶️ Run manually

Open app:

```bash
open -n ~/Applications/MacMousePaste.app
```

Or double-click in Finder.

---

# 🔐 Required Permissions

macOS requires permissions for global input monitoring.

Go to:

System Settings → Privacy & Security

Enable for **MacMousePaste.app**:

### Required

- Accessibility

### Recommended

- Input Monitoring

Important:

Enable permissions for:

```
~/Applications/MacMousePaste.app
```

Not Terminal or target folder.

---

# 🚀 Autorun behavior

Start at Login is enabled automatically by install.sh.

LaunchAgent location:

```
~/Library/LaunchAgents/com.vachagan.macmousepaste.plist
```

View status:

```bash
UID=$(id -u)
launchctl print gui/$UID/com.vachagan.macmousepaste
```

---

# 🛑 Stop and uninstall

Uninstall:

```bash
./scripts/uninstall.sh
```

Removes:

- app bundle
- autorun configuration

---

# 🧪 Development workflow

Typical loop:

```bash
./scripts/build_app.sh
./scripts/install.sh
```

Restart running app:

```bash
pkill -f MacMousePaste
open -n ~/Applications/MacMousePaste.app
```

---

# 🐞 Troubleshooting

## App cannot be opened

Run:

```bash
xattr -dr com.apple.quarantine ~/Applications/MacMousePaste.app
```

---

## Selection not detected

Check permissions:

Accessibility
Input Monitoring

Restart app after granting permissions.

---

## App starts twice

Kill existing instance:

```bash
pkill -f MacMousePaste
```

---

## Logs

Check logs:

```bash
tail -f /tmp/macmousepaste.err.log
```

---

# 🏗️ Architecture

```
src/
  main.rs
    Menu bar UI

  engine.rs
    Event tap
    Copy/paste logic

scripts/
  build_app.sh
  install.sh
  uninstall.sh
```

---

# 🔒 Security

MacMousePaste uses:

- Quartz Event Tap
- Clipboard access
- Synthetic keyboard events

Permissions are required by macOS for safety.

App runs locally only.

No network access.

No data collection.

---

# 📜 License

Public domain or MIT — your choice.

---

# ❤️ Summary

MacMousePaste brings Linux middle-click paste to macOS with:

- clean menu bar UI
- clipboard preservation
- autorun support
- Rust native implementation

Fast. Lightweight. Reliable.

