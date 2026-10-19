//! Taking over a physical screen for a remote session (`--takeover DP-1`).
//!
//! The session's headless output takes every workspace of the physical
//! screen, so the client sees the desktop at its own resolution. The
//! physical screen keeps a placeholder workspace with the away panel and goes
//! dark ("armed"): its wake-on-input options are off and the local keyboards
//! are disabled, so nobody at the desk can type into the remote session.
//!
//! Hyprland cannot tell local input from gliff's virtual devices, so a local
//! mouse is recognised by its effect: the shared cursor moves while the
//! client has sent nothing. That wakes the screen and locks the session (the
//! lock covers every output, the client's too); after an unlock the panel
//! offers to reclaim the screen or turn it off again.
//!
//! Everything changed is written to a state file first. A guard process
//! restores it if the server dies without doing so itself, and
//! `gliff-server --reclaim` restores a leftover state by hand.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use hypr_ipc::{Action, Flavor, Instance};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

/// The placeholder workspace left on the physical screen.
const AWAY_WORKSPACE: &str = "name:gliff-away";
/// Options that would let the client's own input wake the dark screen.
const WAKE_OPTIONS: [&str; 2] = [
    "misc:key_press_enables_dpms",
    "misc:mouse_move_enables_dpms",
];
/// Locks with the desktop's own locker where there is one.
pub const DEFAULT_LOCK: &str =
    "command -v omarchy-system-lock >/dev/null && omarchy-system-lock || loginctl lock-session";
/// How often the armed state is checked against the compositor.
const TICK: Duration = Duration::from_secs(2);
/// How far left of every screen the headless output's left edge sits. It
/// goes left because Hyprland re-places auto-positioned screens to the right
/// of an explicitly placed one; the gap stays wider than any headless output
/// (4096 pixels at scale 0.5), so the local mouse cannot cross onto it.
const LAYOUT_OFFSET: i32 = 16384;

/// What is changed, kept on disk so it can be put back after a crash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct State {
    pub physical: String,
    pub headless: String,
    /// The physical screen's workspaces, as selectors, in their order.
    pub workspaces: Vec<String>,
    /// The workspace the physical screen showed.
    pub active: Option<String>,
    /// The wake options' values before the session.
    pub wake: Vec<(String, bool)>,
    /// Keyboards disabled while armed.
    pub disabled: Vec<String>,
    pub armed: bool,
    /// Run when the screen is given back while still dark.
    #[serde(default)]
    pub lock: String,
}

/// Requests from the away panel, `--reclaim`, and the compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Reclaim,
    /// Turn the physical screen off again (after an unlock at the desk).
    Off,
    /// The config was reloaded, which resets options and device rules.
    Reloaded,
    Tick,
}

pub struct Settings {
    pub physical: String,
    pub lock: String,
    /// Shown on the away panel: where the session comes from.
    pub peer: String,
}

pub struct Takeover {
    instance: Instance,
    flavor: Flavor,
    lock: String,
    state: State,
    state_path: PathBuf,
    socket_path: PathBuf,
    detector: WakeDetector,
    /// The guard's stdin; closing it (by exiting) wakes the guard.
    guard: Option<Child>,
    restored: bool,
}

/// Where the headless output goes: well left of every screen.
pub fn headless_position(instance: &Instance) -> Result<(i32, i32)> {
    let left = instance
        .monitors()?
        .iter()
        .filter(|m| !m.disabled)
        .map(|m| m.x)
        .min()
        .unwrap_or(0);
    Ok((left - LAYOUT_OFFSET, 0))
}

/// Check that the physical screen can be taken over, before anything is
/// created for it.
pub fn check(instance: &Instance, physical: &str) -> Result<()> {
    let mons = instance.monitors()?;
    let m = mons
        .iter()
        .find(|m| m.name == physical)
        .with_context(|| format!("no output {physical}"))?;
    anyhow::ensure!(!m.disabled, "output {physical} is disabled");
    if let Ok(state) = load(&state_path(physical)?) {
        anyhow::bail!(
            "{physical} is already taken over by {}; run `gliff-server --reclaim {physical}` if that session is gone",
            state.headless
        );
    }
    Ok(())
}

impl Takeover {
    /// Move the physical screen's workspaces to `headless`, leave the
    /// placeholder and the panel, and turn the physical screen off.
    pub fn start(
        instance: &Instance,
        settings: &Settings,
        headless: &str,
        signature: &str,
    ) -> Result<(Self, UnboundedReceiver<Event>)> {
        let flavor = instance.flavor()?;
        let physical = settings.physical.clone();
        let mons = instance.monitors()?;
        let active = mons
            .iter()
            .find(|m| m.name == physical)
            .and_then(|m| m.active_workspace.as_ref())
            .map(|w| hypr_ipc::selector(w.id, &w.name));
        let workspaces: Vec<String> = instance
            .workspaces()?
            .into_iter()
            .filter(|w| w.monitor == physical && w.id > 0)
            .map(|w| w.selector())
            .collect();
        let wake = WAKE_OPTIONS
            .iter()
            .map(|o| {
                let v = instance
                    .get_option(o)
                    .ok()
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                (o.to_string(), v)
            })
            .collect();
        let state_path = state_path(&physical)?;
        let socket_path = socket_path(&physical)?;
        let state = State {
            physical: physical.clone(),
            headless: headless.to_owned(),
            workspaces,
            active,
            wake,
            disabled: Vec::new(),
            armed: false,
            lock: settings.lock.clone(),
        };
        let mut takeover = Self {
            instance: instance.clone(),
            flavor,
            lock: settings.lock.clone(),
            state,
            state_path,
            socket_path,
            detector: WakeDetector::new(Instant::now()),
            guard: None,
            restored: false,
        };
        // From here on Drop restores, whatever fails.
        takeover.save()?;
        takeover.guard = spawn_guard(&takeover.state_path, signature)
            .map_err(|e| tracing::warn!(error = %e, "no takeover guard; a crash would need `gliff-server --reclaim`"))
            .ok();
        let (tx, rx) = mpsc::unbounded_channel();
        listen(&takeover.socket_path, tx.clone())?;
        watch_events(instance, tx.clone());
        tick(tx);

        takeover.run(&Action::FocusMonitor(&physical));
        takeover.run(&Action::FocusWorkspace(AWAY_WORKSPACE));
        for ws in takeover.state.workspaces.clone() {
            if let Err(e) = instance.run(
                flavor,
                &Action::MoveWorkspace {
                    workspace: &ws,
                    monitor: headless,
                },
            ) {
                tracing::warn!(workspace = %ws, error = %e, "could not move a workspace");
            }
        }
        if let Some(ws) = takeover.state.active.clone() {
            takeover.run(&Action::FocusWorkspace(&ws));
        } else {
            takeover.run(&Action::FocusMonitor(headless));
        }
        takeover.spawn_panel(&settings.peer);
        takeover.arm()?;
        tracing::info!(
            physical = %physical,
            headless,
            workspaces = takeover.state.workspaces.len(),
            "took over the screen"
        );
        Ok((takeover, rx))
    }

    /// The client sent input: cursor moves for a while after it are its own.
    pub fn note_input(&mut self) {
        self.detector.on_input(Instant::now());
    }

    /// The cursor moved on the headless output. Returns true when that woke
    /// the physical screen.
    pub fn on_cursor(&mut self, x: f64, y: f64) -> bool {
        if !self.state.armed || !self.detector.on_cursor(Instant::now(), x, y) {
            return false;
        }
        tracing::info!("local mouse moved; waking the screen and locking");
        if let Err(e) = self.wake() {
            tracing::warn!(error = %e, "could not wake the screen");
        }
        true
    }

    pub fn on_event(&mut self, ev: Event) -> Result<()> {
        match ev {
            // The session ends itself on a reclaim, which restores.
            Event::Reclaim => Ok(()),
            Event::Off => {
                if !self.state.armed {
                    tracing::info!("turning the screen off again");
                    self.run(&Action::FocusMonitor(&self.state.headless.clone()));
                    self.arm()?;
                }
                Ok(())
            }
            Event::Reloaded if self.state.armed => {
                tracing::info!("config reloaded; re-applying the armed state");
                self.state.disabled.clear();
                self.arm()
            }
            Event::Tick if self.state.armed => self.enforce(),
            Event::Reloaded | Event::Tick => Ok(()),
        }
    }

    /// Turn the physical screen off and the local keyboards with it.
    fn arm(&mut self) -> Result<()> {
        for (option, _) in self.state.wake.clone() {
            self.request(&hypr_ipc::set_bool(self.flavor, &option, false));
        }
        self.disable_keyboards()?;
        self.run(&Action::Dpms {
            monitor: &self.state.physical.clone(),
            on: false,
        });
        self.state.armed = true;
        self.detector = WakeDetector::new(Instant::now());
        self.save()
    }

    /// While armed: keep the screen dark (an idle daemon may turn every
    /// screen on) and disable keyboards plugged in since.
    fn enforce(&mut self) -> Result<()> {
        self.disable_keyboards()?;
        let lit = self
            .instance
            .monitors()?
            .iter()
            .any(|m| m.name == self.state.physical && m.dpms_status);
        if lit {
            tracing::debug!("the screen came back on; turning it off");
            self.run(&Action::Dpms {
                monitor: &self.state.physical.clone(),
                on: false,
            });
        }
        Ok(())
    }

    fn disable_keyboards(&mut self) -> Result<()> {
        let known: BTreeSet<String> = self.state.disabled.iter().cloned().collect();
        let mut changed = false;
        for kb in self.instance.keyboards()? {
            if kb.is_virtual() || known.contains(&kb.name) {
                continue;
            }
            match hypr_ipc::set_device_enabled(self.flavor, &kb.name, false) {
                Some(cmd) => {
                    self.request(&cmd);
                    self.state.disabled.push(kb.name);
                    changed = true;
                }
                None => tracing::warn!(name = %kb.name, "cannot disable a keyboard with this name"),
            }
        }
        if changed {
            self.save()?;
        }
        Ok(())
    }

    /// Light the physical screen, show the placeholder and lock.
    fn wake(&mut self) -> Result<()> {
        restore_input(&self.instance, self.flavor, &self.state);
        self.state.disabled.clear();
        self.state.armed = false;
        self.save()?;
        let physical = self.state.physical.clone();
        self.run(&Action::Dpms {
            monitor: &physical,
            on: true,
        });
        self.run(&Action::FocusMonitor(&physical));
        if let Some(m) = self
            .instance
            .monitors()?
            .into_iter()
            .find(|m| m.name == physical)
        {
            let (w, h) = m.logical_size();
            self.run(&Action::MoveCursor {
                x: m.x + w / 2,
                y: m.y + h / 2,
            });
        }
        self.run(&Action::FocusWorkspace(AWAY_WORKSPACE));
        let lock = self.lock.clone();
        self.run(&Action::Exec {
            command: &lock,
            workspace: None,
        });
        Ok(())
    }

    fn spawn_panel(&self, peer: &str) {
        let Some(gliff) = gliff_binary() else {
            tracing::warn!("no gliff binary next to gliff-server; the screen gets no away panel");
            return;
        };
        let command = format!(
            "{} --away-panel {} {}",
            shell_quote(&gliff.to_string_lossy()),
            shell_quote(&self.socket_path.to_string_lossy()),
            shell_quote(peer)
        );
        self.run(&Action::Exec {
            command: &command,
            workspace: Some(AWAY_WORKSPACE),
        });
    }

    /// Put everything back except the headless output, which the session
    /// removes once its capture has stopped.
    pub fn restore(&mut self) {
        if self.restored {
            return;
        }
        self.restored = true;
        restore(&self.instance, self.flavor, &self.state);
        let _ = std::fs::remove_file(&self.socket_path);
        let _ = std::fs::remove_file(&self.state_path);
        tracing::info!(physical = %self.state.physical, "gave the screen back");
    }

    fn save(&self) -> Result<()> {
        save(&self.state_path, &self.state)
    }

    fn run(&self, action: &Action) {
        if let Err(e) = self.instance.run(self.flavor, action) {
            tracing::warn!(?action, error = %e, "compositor command failed");
        }
    }

    fn request(&self, command: &str) {
        match self.instance.request(command) {
            Ok(r) if r.trim() == "ok" || r.trim().is_empty() => {}
            Ok(r) => tracing::warn!(command, response = r.trim(), "compositor command failed"),
            Err(e) => tracing::warn!(command, error = %e, "compositor command failed"),
        }
    }
}

impl Drop for Takeover {
    fn drop(&mut self) {
        self.restore();
        // The guard sees its stdin close and finds no state left.
        if let Some(mut guard) = self.guard.take() {
            drop(guard.stdin.take());
            let _ = guard.wait();
        }
    }
}

/// Re-enable the keyboards and the wake options.
fn restore_input(instance: &Instance, flavor: Flavor, state: &State) {
    for name in &state.disabled {
        if let Some(cmd) = hypr_ipc::set_device_enabled(flavor, name, true) {
            let _ = instance.request(&cmd);
        }
    }
    for (option, value) in &state.wake {
        let _ = instance.request(&hypr_ipc::set_bool(flavor, option, *value));
    }
}

/// Everything but removing the headless output: input back on and the
/// workspaces back in place. A screen nobody has unlocked since it went dark
/// stays dark and the session locks, so the desk wakes to the lock screen;
/// one unlocked at the desk lights up. Safe to run twice.
fn restore(instance: &Instance, flavor: Flavor, state: &State) {
    restore_input(instance, flavor, state);
    let run = |action: &Action| {
        if let Err(e) = instance.run(flavor, action) {
            tracing::warn!(?action, error = %e, "compositor command failed");
        }
    };
    if !state.armed {
        run(&Action::Dpms {
            monitor: &state.physical,
            on: true,
        });
    }
    // Workspaces made during the session come back too, after the originals.
    let mut selectors = state.workspaces.clone();
    if let Ok(all) = instance.workspaces() {
        for w in all {
            let sel = w.selector();
            if w.monitor == state.headless && w.id > 0 && !selectors.contains(&sel) {
                selectors.push(sel);
            }
        }
    }
    for ws in &selectors {
        let _ = instance.run(
            flavor,
            &Action::MoveWorkspace {
                workspace: ws,
                monitor: &state.physical,
            },
        );
    }
    run(&Action::FocusMonitor(&state.physical));
    if let Some(ws) = &state.active {
        run(&Action::FocusWorkspace(ws));
    }
    if state.armed && !state.lock.is_empty() {
        run(&Action::Exec {
            command: &state.lock,
            workspace: None,
        });
    }
}

/// Restore a state left by a session that is gone, headless output included.
pub fn recover(instance: &Instance, path: &Path) -> Result<()> {
    let state = load(path)?;
    let flavor = instance.flavor()?;
    tracing::info!(physical = %state.physical, headless = %state.headless, "restoring a takeover");
    restore(instance, flavor, &state);
    let _ = instance.remove_output(&state.headless);
    std::fs::remove_file(path).ok();
    if let Ok(sock) = socket_path(&state.physical) {
        std::fs::remove_file(sock).ok();
    }
    Ok(())
}

/// `gliff-server --reclaim [OUTPUT]`: ask the live session to end, or
/// restore what a dead one left.
pub fn reclaim(instance: &Instance, physical: Option<&str>) -> Result<()> {
    let states = match physical {
        Some(p) => vec![state_path(p)?],
        None => std::fs::read_dir(state_dir()?)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect(),
    };
    anyhow::ensure!(!states.is_empty(), "no screen is taken over");
    for path in states {
        let state = load(&path).with_context(|| format!("read {}", path.display()))?;
        let asked = std::os::unix::net::UnixStream::connect(socket_path(&state.physical)?)
            .and_then(|mut s| s.write_all(b"reclaim\n"));
        match asked {
            Ok(()) => {
                tracing::info!(physical = %state.physical, "asked the session to give the screen back")
            }
            Err(_) => recover(instance, &path)?,
        }
    }
    Ok(())
}

/// `gliff-server --takeover-guard STATE`: wait for the server to exit
/// (stdin closes), then restore whatever it left behind.
pub fn guard(instance: &Instance, path: &Path) -> Result<()> {
    let mut sink = [0u8; 64];
    while std::io::stdin().read(&mut sink).is_ok_and(|n| n > 0) {}
    if path.exists() {
        recover(instance, path)?;
    }
    Ok(())
}

fn spawn_guard(state: &Path, signature: &str) -> Result<Child> {
    use std::os::unix::process::CommandExt;
    let exe = std::env::current_exe()?;
    let child = Command::new(exe)
        .arg("--instance")
        .arg(signature)
        .arg("--takeover-guard")
        .arg(state)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Its own process group, so a hangup that kills the server does not
        // take the guard with it.
        .process_group(0)
        .spawn()?;
    Ok(child)
}

/// Accept the panel's and `--reclaim`'s connections; each line is a request.
fn listen(path: &Path, tx: UnboundedSender<Event>) -> Result<()> {
    let _ = std::fs::remove_file(path);
    let listener =
        tokio::net::UnixListener::bind(path).with_context(|| format!("bind {}", path.display()))?;
    tokio::task::spawn_local(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let conn_tx = tx.clone();
            tokio::task::spawn_local(async move {
                let tx = conn_tx;
                let mut lines = BufReader::new(stream).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let ev = match line.trim() {
                        "reclaim" => Event::Reclaim,
                        "off" => Event::Off,
                        _ => continue,
                    };
                    if tx.send(ev).is_err() {
                        break;
                    }
                }
            });
            if tx.is_closed() {
                break;
            }
        }
    });
    Ok(())
}

fn watch_events(instance: &Instance, tx: UnboundedSender<Event>) {
    let events = match instance.events() {
        Ok(events) => events,
        Err(e) => {
            tracing::warn!(error = %e, "no compositor events; a config reload re-enables the keyboards");
            return;
        }
    };
    let spawned = std::thread::Builder::new()
        .name("gliff-takeover-events".into())
        .spawn(move || {
            for line in events {
                let Ok(line) = line else { break };
                if line.starts_with("configreloaded>>") && tx.send(Event::Reloaded).is_err() {
                    break;
                }
            }
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "compositor event thread");
    }
}

fn tick(tx: UnboundedSender<Event>) {
    tokio::task::spawn_local(async move {
        let mut every = tokio::time::interval(TICK);
        every.tick().await;
        loop {
            every.tick().await;
            if tx.send(Event::Tick).is_err() {
                break;
            }
        }
    });
}

/// Recognises a local mouse by cursor movement the client did not cause.
/// The client's own input can move the cursor indirectly (a workspace switch
/// warps it), so movement within `INPUT_SETTLE` of client input is its own.
#[derive(Debug)]
pub struct WakeDetector {
    armed_at: Instant,
    last_input: Option<Instant>,
    last_pos: Option<(f64, f64)>,
}

impl WakeDetector {
    /// Compositor reactions to the client's input settle within this.
    const INPUT_SETTLE: Duration = Duration::from_millis(500);
    /// Arming moves workspaces and warps the cursor; ignore that.
    const ARM_SETTLE: Duration = Duration::from_secs(1);

    pub fn new(now: Instant) -> Self {
        Self {
            armed_at: now,
            last_input: None,
            last_pos: None,
        }
    }

    pub fn on_input(&mut self, now: Instant) {
        self.last_input = Some(now);
    }

    /// True when this cursor position came from the local mouse.
    pub fn on_cursor(&mut self, now: Instant, x: f64, y: f64) -> bool {
        let previous = self.last_pos.replace((x, y));
        if now.duration_since(self.armed_at) < Self::ARM_SETTLE {
            return false;
        }
        if self
            .last_input
            .is_some_and(|t| now.duration_since(t) < Self::INPUT_SETTLE)
        {
            return false;
        }
        previous.is_some_and(|(px, py)| (x - px).abs() + (y - py).abs() >= 1.0)
    }
}

fn state_dir() -> Result<PathBuf> {
    let dir = hypr_ipc::runtime_dir()?.join("gliff");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn file_stem(physical: &str) -> String {
    physical
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn state_path(physical: &str) -> Result<PathBuf> {
    Ok(state_dir()?.join(format!("takeover-{}.json", file_stem(physical))))
}

fn socket_path(physical: &str) -> Result<PathBuf> {
    Ok(state_dir()?.join(format!("takeover-{}.sock", file_stem(physical))))
}

fn save(path: &Path, state: &State) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(state)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn load(path: &Path) -> Result<State> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

/// The client binary installed beside this one, which draws the away panel.
fn gliff_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let sibling = exe.parent()?.join("gliff");
    sibling.is_file().then_some(sibling)
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Where the session comes from, for the away panel.
pub fn peer_name() -> String {
    std::env::var("SSH_CLIENT")
        .ok()
        .and_then(|v| v.split_whitespace().next().map(str::to_owned))
        .unwrap_or_else(|| "another machine".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_input_explains_cursor_moves() {
        let t0 = Instant::now();
        let mut d = WakeDetector::new(t0);
        let at = |ms| t0 + Duration::from_millis(ms);
        // Arming settles first: moves then are ignored.
        assert!(!d.on_cursor(at(100), 10.0, 10.0));
        assert!(!d.on_cursor(at(1200), 10.0, 10.0));
        // A move shortly after client input is the client's (or a warp it
        // caused).
        d.on_input(at(2000));
        assert!(!d.on_cursor(at(2010), 50.0, 50.0));
        assert!(!d.on_cursor(at(2400), 900.0, 400.0));
        // Unexplained movement is the local mouse.
        assert!(d.on_cursor(at(3000), 905.0, 400.0));
    }

    #[test]
    fn a_still_cursor_never_wakes() {
        let t0 = Instant::now();
        let mut d = WakeDetector::new(t0);
        let later = t0 + Duration::from_secs(5);
        assert!(!d.on_cursor(later, 20.0, 20.0));
        assert!(!d.on_cursor(later, 20.0, 20.0));
        assert!(!d.on_cursor(later, 20.4, 20.0));
    }

    #[test]
    fn state_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.json");
        let state = State {
            physical: "DP-1".into(),
            headless: "HEADLESS-2".into(),
            workspaces: vec!["1".into(), "name:mail".into()],
            active: Some("1".into()),
            wake: vec![("misc:key_press_enables_dpms".into(), true)],
            disabled: vec!["usb-keyboard".into()],
            armed: true,
            lock: "loginctl lock-session".into(),
        };
        save(&path, &state).unwrap();
        assert_eq!(load(&path).unwrap(), state);
    }

    #[test]
    fn quotes_for_the_shell() {
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(file_stem("DP-1"), "DP-1");
        assert_eq!(file_stem("../x"), "___x");
    }
}
