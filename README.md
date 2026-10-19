# gliff

[![CI](https://github.com/omacom/gliff/actions/workflows/ci.yml/badge.svg)](https://github.com/omacom/gliff/actions/workflows/ci.yml)

gliff is a remote desktop for Hyprland. It shows the Hyprland session of
another machine in a window on yours, and all it needs between the two is
ssh: no open ports, no accounts, no relay.

Text stays sharp. Most remote desktops send video with halved colour
resolution, which blurs coloured text; gliff sends full-resolution colour
(4:4:4) through the H.264 hardware every GPU has. Video runs on the GPU at
both ends, and falls back to the CPU on machines where it cannot.

![A gliff window showing a terminal on a remote machine](.github/assets/screenshot.png)

## What you get

- Mirror any screen of the remote machine, or get a private screen that
  exists only for your session and follows the size of your window.
- Your keyboard layout, not the remote's: keys map the same on both ends.
- A shared clipboard in both directions: text, images, and copied files.
- Quality that adapts to the link, giving up frame rate before sharpness.
- Reconnection when the link drops.
- A window that follows your Omarchy theme.

## Requirements

- Hyprland on both machines, with a session running on the remote one. Your
  machine can also be a Mac: see [Use from a Mac](#use-from-a-mac).
- ssh access from your machine to the remote one.
- For GPU video: an AMD or Intel GPU with its Vulkan and VA-API drivers
  (on Arch, `vulkan-radeon` and `mesa` for AMD, or `vulkan-intel` and
  `intel-media-driver` for Intel). Without them, and on NVIDIA, gliff uses
  the CPU and needs nothing extra.

## Install

Install gliff on both machines: your machine runs `gliff`, the remote one
runs `gliff-server`.

Gliff is currently available on the **edge** channel of the [Omarchy Package
Repository (OPR)](https://github.com/omacom/omarchy-pkgs):

```
omarchy pkg add gliff
```

Then open **Gliff** from the app launcher, or run `gliff` in a terminal.

If your channel reports `target not found: gliff`, or you use Arch without
OPR, build and install the package from source:

```
git clone https://github.com/omacom/gliff.git
cd gliff
bin/install
```

This uses the current checkout, including fixes newer than the last release.

To build from source, see [docs/development.md](docs/development.md).

## Use

```
gliff user@host
```

This mirrors the screen that has focus on the remote machine. To pick a
screen, or to get a private one:

```
gliff --output DP-1 user@host   # mirror a named remote screen
gliff --headless user@host      # a private remote screen, sized and
                                # scaled to this window
gliff --headless --fullscreen user@host
                                # the same, fullscreen from the start
```

A mirrored screen keeps its own size and is fitted into the window. A private
screen follows the window as you resize it, and is removed when you
disconnect.

**Away from the desk.** `--takeover` gives you a real screen's desktop at
this window's resolution:

```
gliff --takeover DP-1 --fullscreen user@host
```

DP-1's workspaces move to a private screen sized to your window, and DP-1
itself goes dark with the machine's keyboards switched off, so nobody at the
desk can type into your session. Moving the desk's mouse wakes DP-1 and locks
the session (your view locks too; unlock from either side). After an unlock,
DP-1 shows a panel to take the screen back, which ends your session, or to
turn it off again. When you disconnect, the workspaces go back to DP-1; if
it was still dark, it stays dark and the session locks. If the server dies,
a guard process puts everything back, and `gliff-server --reclaim` on the
remote does the same by hand. `--lock-cmd` on the server changes the
locker, which defaults to Omarchy's, else `loginctl lock-session`.

Every machine you connect to gets a tab in the title bar. Click a tab to
connect to that machine, or to switch to it: each connection keeps running
while you look at another, so switching is instant. Hover a tab for its
button: a stop square disconnects a running machine, and an X forgets one
that is not running. Drag the tabs to reorder them. The + after the tabs
opens the address bar; type `user@host` and press Enter to add a machine.
A bare `gliff` opens the window with a tab for each machine you have used,
and connects again to the ones that were connected when you closed it; with
no machines yet, it opens with the address bar ready.

**Keyboard.** While the pointer is over the picture, everything you type goes
to the remote machine, window-manager shortcuts included; move the pointer
off it to get your own back. Press `Shift+Esc` to get your shortcuts back
without moving the pointer. `--release-hotkey` changes that key (`ctrl+alt+q`,
`double-escape`, or `none`).

**Clipboard.** Copy on one machine and paste on the other. Nothing is sent
until you paste. A paste that takes more than a second shows its progress
with a cancel button. Pasted files are kept in `~/.cache/gliff/clipboard`.

Clipboard sharing trusts every connected host: a host can request the
advertised clipboard contents without a local paste. Use `gliff --no-clipboard`
to disable sharing in both directions when connecting to an untrusted host.
Local selections marked `x-kde-passwordManagerHint` are not shared.

**Slow links.** gliff measures the link and lowers the frame rate, then the
colour detail, then the resolution, and raises them again when there is
room. Two flags change the video, and you pass them to the server through
`--server-bin`: `--low-bandwidth` sends 4:2:0 colour from the start, and
`--full-chroma` keeps 4:4:4 on a server that encodes on the CPU.

```
gliff --server-bin 'gliff-server --low-bandwidth' user@host
```

## Use from a Mac

`gliff` also runs on macOS; the remote machine still runs Hyprland and
`gliff-server`. Build it with Homebrew's GTK:

```
brew install gtk4 libadwaita adwaita-icon-theme pkgconf
git clone https://github.com/omacom/gliff.git
cd gliff
cargo build --release --locked -p gliff
```

Then run `target/release/gliff`. For a remote screen that matches the Mac's:

```
target/release/gliff --headless --fullscreen user@host
```

In fullscreen the private screen takes the resolution and scale of the
Mac's display (2x on a Retina display), so the remote desktop is laid out
like the Mac's own and drawn pixel for pixel. Displays up to 4096x2304
pixels, which covers every MacBook at its default resolution, stream at
their own resolution. A larger one keeps its layout at a lower scale: a 5K
display, 2560x1440 at 2x, gets a remote screen of 2560x1440 at 1.6x,
stretched to fill it. Leave fullscreen and the remote screen follows the
window again.

On a Mac:

- Video is decoded on the CPU, with 4:2:0 colour: macOS has no VA-API.
- Keys are sent by position, Command as Super and Option as Alt. A Mac has
  no XKB keymap to send, so the remote types with its default US layout.
- macOS keeps Command-Tab, Command-Space and Mission Control for itself.

## If something does not work

- Run `gliff-probe all` on each machine. It checks the compositor, the GPU
  drivers and both video paths, and prints PASS or FAIL for each.
- If ssh cannot find `gliff-server`, give its path:
  `gliff --server-bin /path/to/gliff-server user@host`.
- The server needs the remote user's running Hyprland session, which it
  finds under `XDG_RUNTIME_DIR`, or `/run/user/<uid>` when ssh does not set
  it. If the session is elsewhere, set it through
  `--server-bin 'env XDG_RUNTIME_DIR=/path gliff-server'`.
- To rule out a driver problem, force the CPU on your end with
  `gliff --video cpu user@host`.
- If the window connects but stays black, the remote GPU may allocate
  capture buffers it never fills (a VM's virtio-gpu with virgl, or a GPU the
  compositor does not render on). Have the compositor read the frames back
  instead: `gliff --server-bin 'gliff-server --capture shm' user@host`.
  The server then encodes on the CPU.
- Screens wider than 4096 pixels are streamed at a reduced size and scaled
  back up.

## More

- [docs/architecture.md](docs/architecture.md): how it works, the design
  choices, measurements, and what is not built yet.
- [docs/hardware-quirks.md](docs/hardware-quirks.md): what each GPU driver
  and Hyprland do, and what is tested where.
- [docs/development.md](docs/development.md): building, running on one
  machine, and the tests.

gliff is released under the MIT license.
