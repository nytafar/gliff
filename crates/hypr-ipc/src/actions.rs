//! Compositor commands in both config syntaxes. Hyprland 0.56 runs either a
//! hyprlang config, which takes `keyword` and the classic dispatchers, or a
//! Lua config, which rejects both and takes `eval` and `hl.dsp` dispatchers.

/// Which syntax the running config takes commands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    Hyprlang,
    Lua,
}

/// A dispatcher call. Workspaces are selectors: an id such as `3`, or
/// `name:away` for a named workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action<'a> {
    Dpms {
        monitor: &'a str,
        on: bool,
    },
    MoveWorkspace {
        workspace: &'a str,
        monitor: &'a str,
    },
    FocusMonitor(&'a str),
    /// Switch to a workspace, creating it on the focused monitor if needed.
    FocusWorkspace(&'a str),
    /// Warp the cursor to global layout coordinates.
    MoveCursor {
        x: i32,
        y: i32,
    },
    /// Run a shell command in the compositor's environment, its first
    /// window placed on `workspace` without switching to it.
    Exec {
        command: &'a str,
        workspace: Option<&'a str>,
    },
}

impl Action<'_> {
    /// The request to send on the control socket.
    pub fn request(&self, flavor: Flavor) -> String {
        match flavor {
            Flavor::Hyprlang => format!("dispatch {}", self.hyprlang()),
            Flavor::Lua => format!("dispatch {}", self.lua()),
        }
    }

    fn hyprlang(&self) -> String {
        match self {
            Action::Dpms { monitor, on } => {
                format!("dpms {} {monitor}", if *on { "on" } else { "off" })
            }
            Action::MoveWorkspace { workspace, monitor } => {
                format!("moveworkspacetomonitor {workspace} {monitor}")
            }
            Action::FocusMonitor(monitor) => format!("focusmonitor {monitor}"),
            Action::FocusWorkspace(workspace) => format!("workspace {workspace}"),
            Action::MoveCursor { x, y } => format!("movecursor {x} {y}"),
            Action::Exec {
                command,
                workspace: Some(ws),
            } => format!("exec [workspace {ws} silent] {command}"),
            Action::Exec {
                command,
                workspace: None,
            } => format!("exec {command}"),
        }
    }

    fn lua(&self) -> String {
        match self {
            Action::Dpms { monitor, on } => format!(
                "hl.dsp.dpms({{ action = {}, monitor = {} }})",
                lua_str(if *on { "enable" } else { "disable" }),
                lua_str(monitor)
            ),
            Action::MoveWorkspace { workspace, monitor } => format!(
                "hl.dsp.workspace.move({{ workspace = {}, monitor = {} }})",
                lua_str(workspace),
                lua_str(monitor)
            ),
            Action::FocusMonitor(monitor) => {
                format!("hl.dsp.focus({{ monitor = {} }})", lua_str(monitor))
            }
            Action::FocusWorkspace(workspace) => {
                format!("hl.dsp.focus({{ workspace = {} }})", lua_str(workspace))
            }
            Action::MoveCursor { x, y } => format!("hl.dsp.cursor.move({{ x = {x}, y = {y} }})"),
            Action::Exec {
                command,
                workspace: Some(ws),
            } => format!(
                "hl.dsp.exec_cmd({}, {{ workspace = {} }})",
                lua_str(command),
                lua_str(&format!("{ws} silent"))
            ),
            Action::Exec {
                command,
                workspace: None,
            } => format!("hl.dsp.exec_cmd({})", lua_str(command)),
        }
    }
}

/// Set a boolean config option such as `misc:key_press_enables_dpms`.
pub fn set_bool(flavor: Flavor, option: &str, value: bool) -> String {
    match flavor {
        Flavor::Hyprlang => format!("keyword {option} {value}"),
        Flavor::Lua => {
            // `misc:key_press_enables_dpms` is `hl.config({ misc = { ... } })`.
            let mut table = format!("{value}");
            for key in option.rsplit(':') {
                table = format!("{{ {key} = {table} }}");
            }
            format!("eval hl.config({table})")
        }
    }
}

/// Enable or disable one input device by its Hyprland name. `None` when the
/// name cannot be written safely in this syntax.
pub fn set_device_enabled(flavor: Flavor, name: &str, enabled: bool) -> Option<String> {
    match flavor {
        Flavor::Hyprlang => {
            let plain = !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.:".contains(c));
            plain.then(|| format!("keyword device[{name}]:enabled {enabled}"))
        }
        Flavor::Lua => Some(format!(
            "eval hl.device({{ name = {}, enabled = {enabled} }})",
            lua_str(name)
        )),
    }
}

/// A Lua string literal. Device names come from USB descriptors, so every
/// byte that could end the literal or the statement is escaped.
pub fn lua_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for b in s.bytes() {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\{b:03}")),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_both_syntaxes() {
        let off = Action::Dpms {
            monitor: "DP-1",
            on: false,
        };
        assert_eq!(off.request(Flavor::Hyprlang), "dispatch dpms off DP-1");
        assert_eq!(
            off.request(Flavor::Lua),
            r#"dispatch hl.dsp.dpms({ action = "disable", monitor = "DP-1" })"#
        );
        let mv = Action::MoveWorkspace {
            workspace: "3",
            monitor: "HEADLESS-2",
        };
        assert_eq!(
            mv.request(Flavor::Hyprlang),
            "dispatch moveworkspacetomonitor 3 HEADLESS-2"
        );
        assert_eq!(
            mv.request(Flavor::Lua),
            r#"dispatch hl.dsp.workspace.move({ workspace = "3", monitor = "HEADLESS-2" })"#
        );
        let exec = Action::Exec {
            command: "gliff away-panel",
            workspace: Some("name:away"),
        };
        assert_eq!(
            exec.request(Flavor::Hyprlang),
            "dispatch exec [workspace name:away silent] gliff away-panel"
        );
        assert_eq!(
            exec.request(Flavor::Lua),
            r#"dispatch hl.dsp.exec_cmd("gliff away-panel", { workspace = "name:away silent" })"#
        );
        assert_eq!(
            Action::MoveCursor { x: -5, y: 10 }.request(Flavor::Lua),
            "dispatch hl.dsp.cursor.move({ x = -5, y = 10 })"
        );
    }

    #[test]
    fn sets_nested_options() {
        assert_eq!(
            set_bool(Flavor::Lua, "misc:key_press_enables_dpms", false),
            "eval hl.config({ misc = { key_press_enables_dpms = false } })"
        );
        assert_eq!(
            set_bool(Flavor::Hyprlang, "misc:key_press_enables_dpms", true),
            "keyword misc:key_press_enables_dpms true"
        );
    }

    #[test]
    fn device_names_cannot_escape_the_command() {
        assert_eq!(
            set_device_enabled(Flavor::Lua, "usb-keyboard", false).unwrap(),
            r#"eval hl.device({ name = "usb-keyboard", enabled = false })"#
        );
        assert_eq!(
            set_device_enabled(Flavor::Lua, "a\" }) os.exit() --\n", true).unwrap(),
            r#"eval hl.device({ name = "a\" }) os.exit() --\010", enabled = true })"#
        );
        assert_eq!(
            set_device_enabled(Flavor::Hyprlang, "usb-keyboard-1", false).unwrap(),
            "keyword device[usb-keyboard-1]:enabled false"
        );
        assert!(set_device_enabled(Flavor::Hyprlang, "a]:x", false).is_none());
        assert!(set_device_enabled(Flavor::Hyprlang, "", false).is_none());
    }
}
