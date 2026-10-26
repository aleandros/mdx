//! Startup decision on whether mdx may draw pixel graphics.
//!
//! The setting (`--graphics`, `MDX_GRAPHICS`, `graphics` in the config) is
//! resolved once, and in `auto` mode the terminal is asked whether it speaks
//! the Kitty graphics protocol. Every graphics feature reads the resulting
//! [`GraphicsCaps`]; nothing here changes rendering.

use std::io::IsTerminal;
use std::time::Duration;

use serde::Deserialize;

/// Environment variable that overrides the config file (but not the CLI flag).
pub(crate) const ENV_VAR: &str = "MDX_GRAPHICS";

/// Image id carried by the query; the terminal echoes it in its reply.
const PROBE_IMAGE_ID: u32 = 0x6d64;

/// Longest wait for the DA1 reply, which comes last and which terminals
/// send within milliseconds; the probe returns as soon as it arrives. Only a
/// terminal that never answers DA1 waits this long. A shorter timeout would
/// let a slow reply arrive after the probe, where the pager would read it as
/// keystrokes.
const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

/// [`PROBE_TIMEOUT`] over SSH, where the round trip can be much longer.
const PROBE_TIMEOUT_SSH: Duration = Duration::from_secs(2);

/// Terminals whose Unicode placeholder support has been verified, with the
/// first version that has it, matched against the `XTVERSION` reply.
/// Ghostty, `WezTerm` and Konsole are candidates, not yet verified.
const PLACEHOLDER_ALLOWLIST: &[(&str, (u32, u32))] = &[("kitty", (0, 28))];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub(crate) enum GraphicsMode {
    /// Draw graphics when the terminal supports them
    Auto,
    /// Never draw graphics
    Off,
}

/// Where the effective [`GraphicsMode`] came from, for diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingSource {
    Default,
    Config,
    Env,
    Cli,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GraphicsSetting {
    pub(crate) mode: GraphicsMode,
    pub(crate) source: SettingSource,
}

/// Where the rendered document goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputTarget {
    /// The interactive pager (including watch mode).
    Pager,
    /// Plain stdout: piped or `--no-pager` output.
    Plain,
}

/// What mdx may draw, decided once at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GraphicsCaps {
    pub(crate) supported: bool,
    /// Cell size in pixels as (width, height); (0, 0) when unknown.
    pub(crate) cell_px: (u16, u16),
    pub(crate) via_tmux: bool,
    pub(crate) reason: &'static str,
}

impl GraphicsCaps {
    pub(crate) const fn unsupported(reason: &'static str) -> Self {
        GraphicsCaps {
            supported: false,
            cell_px: (0, 0),
            via_tmux: false,
            reason,
        }
    }
}

/// The terminal's answer to the Kitty graphics query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GraphicsReply {
    /// No graphics reply arrived before the DA1 reply.
    NoAnswer,
    Ok,
    Error(String),
}

/// Replies collected up to and including the DA1 reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProbeReply {
    pub(crate) graphics: GraphicsReply,
    /// `XTVERSION` text, e.g. `kitty(0.35.2)`.
    pub(crate) version: Option<String>,
    /// Cell size from `CSI 16 t` as (width, height).
    pub(crate) cell_px: Option<(u16, u16)>,
}

/// What happened to the terminal query.
#[derive(Debug)]
pub(crate) enum Probe {
    /// No query was sent: the decision did not need one.
    NotSent,
    TimedOut,
    Failed(std::io::Error),
    Answered(ProbeReply),
}

/// The parsed reply (`None` on timeout) and the cell size from `TIOCGWINSZ`.
type ProbeOutput = (Option<ProbeReply>, Option<(u16, u16)>);

#[derive(Debug)]
pub(crate) struct Detection {
    pub(crate) caps: GraphicsCaps,
    pub(crate) probe: Probe,
}

fn parse_mode(value: &str) -> Option<GraphicsMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Some(GraphicsMode::Auto),
        "off" => Some(GraphicsMode::Off),
        _ => None,
    }
}

/// Resolve the effective setting: CLI flag, then `MDX_GRAPHICS`, then the
/// config file, then `auto`. An empty environment value counts as unset; an
/// invalid one is ignored and returned as a warning, so a typo cannot break
/// output that never uses graphics.
pub(crate) fn resolve_setting(
    cli: Option<GraphicsMode>,
    env: Option<&str>,
    config: Option<GraphicsMode>,
) -> (GraphicsSetting, Option<String>) {
    if let Some(mode) = cli {
        let setting = GraphicsSetting {
            mode,
            source: SettingSource::Cli,
        };
        return (setting, None);
    }
    let mut warning = None;
    if let Some(value) = env.filter(|v| !v.trim().is_empty()) {
        if let Some(mode) = parse_mode(value) {
            let setting = GraphicsSetting {
                mode,
                source: SettingSource::Env,
            };
            return (setting, None);
        }
        warning = Some(format!(
            "ignoring invalid {ENV_VAR} value {value:?} (expected \"auto\" or \"off\")"
        ));
    }
    let setting = config.map_or(
        GraphicsSetting {
            mode: GraphicsMode::Auto,
            source: SettingSource::Default,
        },
        |mode| GraphicsSetting {
            mode,
            source: SettingSource::Config,
        },
    );
    (setting, warning)
}

/// Resolve the setting from the CLI flag, the process environment and the
/// config, printing a warning for an invalid `MDX_GRAPHICS` value.
pub(crate) fn setting_from_env(
    cli: Option<GraphicsMode>,
    config: Option<GraphicsMode>,
) -> GraphicsSetting {
    let env = std::env::var(ENV_VAR).ok();
    let (setting, warning) = resolve_setting(cli, env.as_deref(), config);
    if let Some(warning) = warning {
        eprintln!("mdx: warning: {warning}");
    }
    setting
}

/// Decide everything that needs no terminal query. `None` means the terminal
/// must be asked.
#[expect(
    clippy::fn_params_excessive_bools,
    reason = "pure decision over two environment facts, kept separate for tests"
)]
fn decide_without_probe(
    setting: GraphicsSetting,
    target: OutputTarget,
    stdout_is_terminal: bool,
    in_tmux: bool,
) -> Option<GraphicsCaps> {
    if setting.mode == GraphicsMode::Off {
        return Some(GraphicsCaps::unsupported(match setting.source {
            SettingSource::Cli => "disabled by --graphics off",
            SettingSource::Env => "disabled by MDX_GRAPHICS=off",
            SettingSource::Config | SettingSource::Default => {
                "disabled by graphics = \"off\" in the config file"
            }
        }));
    }
    if target != OutputTarget::Pager || !stdout_is_terminal {
        return Some(GraphicsCaps::unsupported(
            "output is not the interactive pager on a terminal",
        ));
    }
    if in_tmux {
        return Some(GraphicsCaps {
            via_tmux: true,
            ..GraphicsCaps::unsupported(
                "running inside tmux; tmux passthrough is not supported yet",
            )
        });
    }
    None
}

/// Decide from the terminal's replies. `None` means the query timed out.
fn decide_from_reply(reply: Option<&ProbeReply>) -> (bool, &'static str) {
    let Some(reply) = reply else {
        return (false, "terminal did not answer the query in time");
    };
    match &reply.graphics {
        GraphicsReply::NoAnswer => (
            false,
            "terminal does not support the Kitty graphics protocol",
        ),
        GraphicsReply::Error(_) => (false, "terminal rejected the Kitty graphics query"),
        GraphicsReply::Ok if placeholders_known(reply.version.as_deref()) => (
            true,
            "terminal supports Kitty graphics with Unicode placeholders",
        ),
        GraphicsReply::Ok => (
            false,
            "terminal answered the graphics query, but its Unicode placeholder support is not verified",
        ),
    }
}

/// Parse an `XTVERSION` reply such as `kitty(0.35.2)` into its name and
/// (major, minor) version.
fn parse_version(text: &str) -> Option<(&str, (u32, u32))> {
    let (name, rest) = text.split_once('(')?;
    let version = rest.strip_suffix(')')?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((name.trim(), (major, minor)))
}

fn placeholders_known(version: Option<&str>) -> bool {
    let Some((name, version)) = version.and_then(parse_version) else {
        return false;
    };
    PLACEHOLDER_ALLOWLIST
        .iter()
        .any(|&(allowed, minimum)| name.eq_ignore_ascii_case(allowed) && version >= minimum)
}

/// The bytes sent to the terminal: the graphics query, `XTVERSION`, the cell
/// size request and finally DA1, which every terminal answers.
fn query_bytes(id: u32) -> Vec<u8> {
    format!("\x1b_Gi={id},s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[>q\x1b[16t\x1b[c").into_bytes()
}

const ESC: u8 = 0x1b;

/// How a string sequence (APC/DCS) starting at some offset ends.
enum StringEnd {
    /// Index of the ESC of the terminating ST.
    Terminated(usize),
    /// The buffer ends first; more bytes may complete it.
    Incomplete,
    /// Another escape sequence starts first, so this was not a reply.
    Interrupted,
}

fn find_st(buf: &[u8], from: usize) -> StringEnd {
    let Some(rest) = buf.get(from..) else {
        return StringEnd::Incomplete;
    };
    match rest.iter().position(|&b| b == ESC) {
        None => StringEnd::Incomplete,
        Some(p) => match rest.get(p + 1) {
            None => StringEnd::Incomplete,
            Some(b'\\') => StringEnd::Terminated(from + p),
            Some(_) => StringEnd::Interrupted,
        },
    }
}

/// Parse the replies read so far. Returns `None` until the DA1 reply has
/// arrived; anything after it is not part of the reply. Unrelated bytes,
/// such as keys pressed during the query, are skipped: a sequence that is
/// not well formed only skips its ESC.
pub(crate) fn parse_reply(buf: &[u8], id: u32) -> Option<ProbeReply> {
    let mut reply = ProbeReply {
        graphics: GraphicsReply::NoAnswer,
        version: None,
        cell_px: None,
    };
    let mut i = 0;
    while i < buf.len() {
        if buf[i] != ESC {
            i += 1;
            continue;
        }
        match buf.get(i + 1)? {
            // APC (graphics) and DCS (XTVERSION) strings end with ST.
            &kind @ (b'_' | b'P') => {
                let expected = if kind == b'_' { b'G' } else { b'>' };
                if *buf.get(i + 2)? != expected {
                    i += 1;
                    continue;
                }
                let end = match find_st(buf, i + 2) {
                    StringEnd::Terminated(end) => end,
                    StringEnd::Incomplete => return None,
                    StringEnd::Interrupted => {
                        i += 1;
                        continue;
                    }
                };
                let body = String::from_utf8_lossy(&buf[i + 2..end]);
                if let Some(graphics) = body.strip_prefix('G') {
                    parse_graphics(graphics, id, &mut reply);
                } else if let Some(version) = body.strip_prefix(">|") {
                    reply.version = Some(version.to_string());
                }
                i = end + 2;
            }
            b'[' => {
                let start = i + 2;
                // Parameter and intermediate bytes, then one final byte.
                let len = buf
                    .get(start..)?
                    .iter()
                    .position(|b| !(0x20..=0x3f).contains(b))?;
                if !(0x40..=0x7e).contains(&buf[start + len]) {
                    i += 1;
                    continue;
                }
                let params = &buf[start..start + len];
                match buf[start + len] {
                    b'c' if params.first() == Some(&b'?') => return Some(reply),
                    b't' => {
                        if let Some(cell) = parse_cell_size(params) {
                            reply.cell_px = Some(cell);
                        }
                    }
                    _ => {}
                }
                i = start + len + 1;
            }
            _ => i += 1,
        }
    }
    None
}

fn parse_graphics(body: &str, id: u32, reply: &mut ProbeReply) {
    let (keys, message) = body.split_once(';').unwrap_or((body, ""));
    let ours = keys
        .split(',')
        .any(|kv| kv.strip_prefix("i=").and_then(|v| v.parse().ok()) == Some(id));
    if !ours {
        return;
    }
    reply.graphics = if message == "OK" {
        GraphicsReply::Ok
    } else {
        GraphicsReply::Error(message.to_string())
    };
}

/// `CSI 6 ; height ; width t` → (width, height).
fn parse_cell_size(params: &[u8]) -> Option<(u16, u16)> {
    let text = std::str::from_utf8(params).ok()?;
    let mut parts = text.split(';');
    if parts.next()? != "6" {
        return None;
    }
    let height: u16 = parts.next()?.parse().ok()?;
    let width: u16 = parts.next()?.parse().ok()?;
    (width > 0 && height > 0).then_some((width, height))
}

fn env_is_set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| !v.is_empty())
}

/// Decide whether graphics may be drawn, querying the terminal when needed.
/// Sends nothing unless the output is the pager on a terminal and the mode
/// is `auto`. Enables raw mode for the query if it is not enabled already.
pub(crate) fn detect(setting: GraphicsSetting, target: OutputTarget) -> Detection {
    if let Some(caps) = decide_without_probe(
        setting,
        target,
        std::io::stdout().is_terminal(),
        env_is_set("TMUX"),
    ) {
        return Detection {
            caps,
            probe: Probe::NotSent,
        };
    }
    let timeout = if env_is_set("SSH_TTY") || env_is_set("SSH_CONNECTION") {
        PROBE_TIMEOUT_SSH
    } else {
        PROBE_TIMEOUT
    };
    match sys::probe(&query_bytes(PROBE_IMAGE_ID), timeout) {
        Ok((reply, winsize_cell)) => {
            let (supported, reason) = decide_from_reply(reply.as_ref());
            let cell_px = reply
                .as_ref()
                .and_then(|r| r.cell_px)
                .or(winsize_cell)
                .unwrap_or((0, 0));
            Detection {
                caps: GraphicsCaps {
                    supported,
                    cell_px,
                    via_tmux: false,
                    reason,
                },
                probe: reply.map_or(Probe::TimedOut, Probe::Answered),
            }
        }
        Err(err) => Detection {
            caps: GraphicsCaps::unsupported("could not query the terminal"),
            probe: Probe::Failed(err),
        },
    }
}

/// Terminal-supplied text with control characters removed, safe to print.
fn printable(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

/// Human-readable report for `--debug-graphics`.
pub(crate) fn report(setting: GraphicsSetting, detection: &Detection) -> String {
    let caps = &detection.caps;
    let mode = match setting.mode {
        GraphicsMode::Auto => "auto",
        GraphicsMode::Off => "off",
    };
    let source = match setting.source {
        SettingSource::Default => "default",
        SettingSource::Config => "config file",
        SettingSource::Env => ENV_VAR,
        SettingSource::Cli => "--graphics",
    };
    let probe = match &detection.probe {
        Probe::NotSent => "not sent".to_string(),
        Probe::TimedOut => "timed out".to_string(),
        Probe::Failed(err) => format!("failed: {err}"),
        Probe::Answered(reply) => match &reply.graphics {
            GraphicsReply::NoAnswer => "no graphics reply before DA1".to_string(),
            GraphicsReply::Ok => "OK".to_string(),
            GraphicsReply::Error(message) => format!("error: {}", printable(message)),
        },
    };
    let terminal = match &detection.probe {
        Probe::Answered(ProbeReply {
            version: Some(version),
            ..
        }) => printable(version),
        _ => "unknown".to_string(),
    };
    let cell = if caps.cell_px == (0, 0) {
        "unknown".to_string()
    } else {
        format!("{}x{} px", caps.cell_px.0, caps.cell_px.1)
    };
    format!(
        "graphics: {}\nreason: {}\nsetting: {mode} (from {source})\nquery: {probe}\nterminal: {terminal}\ncell size: {cell}\ntmux: {}\n",
        if caps.supported { "on" } else { "off" },
        caps.reason,
        if caps.via_tmux { "yes" } else { "no" },
    )
}

#[cfg(unix)]
mod sys {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    use std::time::{Duration, Instant};

    /// Restores cooked mode if the probe had to enable raw mode itself.
    struct RawModeGuard {
        restore: bool,
    }

    impl Drop for RawModeGuard {
        fn drop(&mut self) {
            if self.restore {
                let _ = crossterm::terminal::disable_raw_mode();
            }
        }
    }

    /// Send `query` to the controlling terminal and read replies until DA1.
    /// Returns the parsed reply (`None` when nothing complete arrived in
    /// time) and the cell size from `TIOCGWINSZ`. Input that is not part of
    /// the reply, such as keys typed during the query, is consumed and lost.
    pub(super) fn probe(query: &[u8], timeout: Duration) -> std::io::Result<super::ProbeOutput> {
        let mut tty = File::options().read(true).write(true).open("/dev/tty")?;
        let already_raw = crossterm::terminal::is_raw_mode_enabled()?;
        if !already_raw {
            crossterm::terminal::enable_raw_mode()?;
        }
        let _guard = RawModeGuard {
            restore: !already_raw,
        };
        let winsize_cell = winsize_cell(&tty);
        let reply = exchange(&mut tty, query, timeout)?;
        Ok((reply, winsize_cell))
    }

    /// Write `query` to `tty` (already in raw mode) and read until a complete
    /// reply arrives or `timeout` passes. Without a complete reply, queued
    /// input is dropped so the pager (or the shell, after --debug-graphics)
    /// does not read a partial reply as keystrokes.
    pub(super) fn exchange(
        tty: &mut File,
        query: &[u8],
        timeout: Duration,
    ) -> std::io::Result<Option<super::ProbeReply>> {
        // Fail before sending anything, so no reply can arrive unread.
        check_selectable(tty)?;
        tty.write_all(query)?;
        tty.flush()?;
        let mut buf = Vec::new();
        let reply = read_reply(tty, &mut buf, Instant::now() + timeout);
        if !matches!(reply, Ok(Some(_))) {
            // SAFETY: tcflush only discards queued input on a valid, open fd.
            unsafe {
                libc::tcflush(tty.as_raw_fd(), libc::TCIFLUSH);
            }
        }
        reply
    }

    /// Read into `buf` until it holds a complete reply or `deadline` passes.
    fn read_reply(
        tty: &mut File,
        buf: &mut Vec<u8>,
        deadline: Instant,
    ) -> std::io::Result<Option<super::ProbeReply>> {
        let mut chunk = [0u8; 256];
        loop {
            if let Some(reply) = super::parse_reply(buf, super::PROBE_IMAGE_ID) {
                return Ok(Some(reply));
            }
            if !wait_readable(tty, deadline)? {
                return Ok(None);
            }
            let n = match tty.read(&mut chunk) {
                Ok(0) => return Ok(None),
                Ok(n) => n,
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(err),
            };
            buf.extend_from_slice(&chunk[..n]);
        }
    }

    /// `select` only handles descriptors below `FD_SETSIZE`.
    fn check_selectable(tty: &File) -> std::io::Result<()> {
        let fd = tty.as_raw_fd();
        if usize::try_from(fd).is_ok_and(|fd| fd < libc::FD_SETSIZE) {
            Ok(())
        } else {
            Err(std::io::Error::other("tty descriptor too large for select"))
        }
    }

    /// Wait until `tty` has input or `deadline` passes. Uses `select` because
    /// macOS `poll` does not support tty devices.
    pub(super) fn wait_readable(tty: &File, deadline: Instant) -> std::io::Result<bool> {
        check_selectable(tty)?;
        let fd = tty.as_raw_fd();
        loop {
            let timeout = deadline.saturating_duration_since(Instant::now());
            if timeout.is_zero() {
                return Ok(false);
            }
            // SAFETY: fd_set is plain data; FD_ZERO below initialises it.
            let mut set: libc::fd_set = unsafe { std::mem::zeroed() };
            // SAFETY: `set` is a valid, exclusively borrowed fd_set.
            unsafe { libc::FD_ZERO(&raw mut set) };
            // SAFETY: `fd` is below FD_SETSIZE (checked above).
            unsafe { libc::FD_SET(fd, &raw mut set) };
            let mut tv = libc::timeval {
                tv_sec: libc::time_t::try_from(timeout.as_secs()).unwrap_or(libc::time_t::MAX),
                // suseconds_t is i32 on macOS and i64 on Linux; u64 converts
                // fallibly to both.
                tv_usec: libc::suseconds_t::try_from(u64::from(timeout.subsec_micros()))
                    .unwrap_or(0),
            };
            // SAFETY: `set` and `tv` are valid and outlive the call; the
            // write and except sets may be null.
            let ready = unsafe {
                libc::select(
                    fd + 1,
                    &raw mut set,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &raw mut tv,
                )
            };
            if ready >= 0 {
                return Ok(ready > 0);
            }
            let err = std::io::Error::last_os_error();
            if err.kind() != std::io::ErrorKind::Interrupted {
                return Err(err);
            }
        }
    }

    /// Cell size from the pixel fields of `TIOCGWINSZ`, which many terminals
    /// leave at zero.
    fn winsize_cell(tty: &File) -> Option<(u16, u16)> {
        let mut ws = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: TIOCGWINSZ writes one winsize into `ws`, which outlives the call.
        let rc = unsafe { libc::ioctl(tty.as_raw_fd(), libc::TIOCGWINSZ, &raw mut ws) };
        if rc != 0 || ws.ws_col == 0 || ws.ws_row == 0 {
            return None;
        }
        let width = ws.ws_xpixel / ws.ws_col;
        let height = ws.ws_ypixel / ws.ws_row;
        (width > 0 && height > 0).then_some((width, height))
    }
}

#[cfg(not(unix))]
mod sys {
    use std::time::Duration;

    pub(super) fn probe(_query: &[u8], _timeout: Duration) -> std::io::Result<super::ProbeOutput> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "terminal queries are only implemented on Unix",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: u32 = PROBE_IMAGE_ID;
    const DA1: &[u8] = b"\x1b[?62;22c";

    fn graphics_ok() -> Vec<u8> {
        format!("\x1b_Gi={ID};OK\x1b\\").into_bytes()
    }

    fn concat(parts: &[&[u8]]) -> Vec<u8> {
        parts.concat()
    }

    fn setting(mode: GraphicsMode, source: SettingSource) -> GraphicsSetting {
        GraphicsSetting { mode, source }
    }

    #[test]
    fn parse_ok_before_da1() {
        let buf = concat(&[&graphics_ok(), DA1]);
        let reply = parse_reply(&buf, ID).unwrap();
        assert_eq!(reply.graphics, GraphicsReply::Ok);
    }

    #[test]
    fn parse_error_reply() {
        let buf = concat(&[
            format!("\x1b_Gi={ID};ENOTSUPPORTED:bad format\x1b\\").as_bytes(),
            DA1,
        ]);
        let reply = parse_reply(&buf, ID).unwrap();
        assert_eq!(
            reply.graphics,
            GraphicsReply::Error("ENOTSUPPORTED:bad format".to_string())
        );
        assert_eq!(
            decide_from_reply(Some(&reply)),
            (false, "terminal rejected the Kitty graphics query")
        );
    }

    #[test]
    fn parse_no_graphics_answer() {
        let reply = parse_reply(DA1, ID).unwrap();
        assert_eq!(reply.graphics, GraphicsReply::NoAnswer);
        assert!(!decide_from_reply(Some(&reply)).0);
    }

    #[test]
    fn parse_da1_first_ignores_later_graphics_reply() {
        let buf = concat(&[DA1, &graphics_ok()]);
        let reply = parse_reply(&buf, ID).unwrap();
        assert_eq!(reply.graphics, GraphicsReply::NoAnswer);
    }

    #[test]
    fn parse_incomplete_until_da1() {
        assert_eq!(parse_reply(b"", ID), None);
        assert_eq!(parse_reply(&graphics_ok(), ID), None);
        // Truncated mid-sequence.
        assert_eq!(parse_reply(b"\x1b_Gi=1;O", ID), None);
        assert_eq!(parse_reply(b"\x1b[?62;2", ID), None);
        assert_eq!(parse_reply(b"\x1b", ID), None);
    }

    #[test]
    fn timeout_is_unsupported() {
        assert_eq!(
            decide_from_reply(None),
            (false, "terminal did not answer the query in time")
        );
    }

    #[test]
    fn parse_ignores_other_image_ids() {
        let buf = concat(&[b"\x1b_Gi=7;OK\x1b\\", DA1]);
        assert_eq!(
            parse_reply(&buf, ID).unwrap().graphics,
            GraphicsReply::NoAnswer
        );
    }

    #[test]
    fn parse_full_kitty_reply() {
        let buf = concat(&[
            &graphics_ok(),
            b"\x1bP>|kitty(0.35.2)\x1b\\",
            b"\x1b[6;20;10t",
            DA1,
        ]);
        let reply = parse_reply(&buf, ID).unwrap();
        assert_eq!(reply.version.as_deref(), Some("kitty(0.35.2)"));
        assert_eq!(reply.cell_px, Some((10, 20)));
        assert_eq!(
            decide_from_reply(Some(&reply)),
            (
                true,
                "terminal supports Kitty graphics with Unicode placeholders"
            )
        );
    }

    #[test]
    fn parse_skips_unrelated_bytes() {
        let buf = concat(&[b"jk\x1bOA", &graphics_ok(), b"x\x1b[1;5A", DA1]);
        assert_eq!(parse_reply(&buf, ID).unwrap().graphics, GraphicsReply::Ok);
    }

    #[test]
    fn placeholder_allowlist() {
        assert!(placeholders_known(Some("kitty(0.28.0)")));
        assert!(placeholders_known(Some("kitty(1.0.0)")));
        assert!(!placeholders_known(Some("kitty(0.27.1)")));
        assert!(!placeholders_known(Some(
            "WezTerm 20240203-110809-5046fc22"
        )));
        assert!(!placeholders_known(Some("ghostty 1.1.0")));
        assert!(!placeholders_known(None));
    }

    #[test]
    fn graphics_ok_without_allowlisted_terminal_is_off() {
        let buf = concat(&[&graphics_ok(), b"\x1bP>|ghostty 1.1.0\x1b\\", DA1]);
        let reply = parse_reply(&buf, ID).unwrap();
        assert!(!decide_from_reply(Some(&reply)).0);
    }

    #[test]
    fn query_bytes_end_with_da1() {
        let bytes = query_bytes(ID);
        assert!(bytes.starts_with(b"\x1b_Gi=28004,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\"));
        assert!(bytes.ends_with(b"\x1b[c"));
    }

    #[test]
    fn precedence_cli_wins() {
        let s = resolve_setting(
            Some(GraphicsMode::Auto),
            Some("off"),
            Some(GraphicsMode::Off),
        );
        assert_eq!(s, (setting(GraphicsMode::Auto, SettingSource::Cli), None));
    }

    #[test]
    fn precedence_env_over_config() {
        let s = resolve_setting(None, Some("OFF"), Some(GraphicsMode::Auto));
        assert_eq!(s, (setting(GraphicsMode::Off, SettingSource::Env), None));
    }

    #[test]
    fn precedence_config_then_default() {
        let s = resolve_setting(None, None, Some(GraphicsMode::Off));
        assert_eq!(s, (setting(GraphicsMode::Off, SettingSource::Config), None));
        let s = resolve_setting(None, Some(""), None);
        assert_eq!(
            s,
            (setting(GraphicsMode::Auto, SettingSource::Default), None)
        );
    }

    #[test]
    fn invalid_env_value_warns_and_falls_back() {
        let (s, warning) = resolve_setting(None, Some("on"), Some(GraphicsMode::Off));
        assert_eq!(s, setting(GraphicsMode::Off, SettingSource::Config));
        let warning = warning.unwrap();
        assert!(warning.contains("MDX_GRAPHICS"), "{warning}");
        // The CLI flag makes the environment irrelevant.
        let (_, warning) = resolve_setting(Some(GraphicsMode::Off), Some("on"), None);
        assert_eq!(warning, None);
    }

    #[test]
    fn parse_survives_keys_pressed_during_query() {
        // A lone Esc / Alt-[ before the reply must not swallow it.
        let buf = concat(&[b"\x1b[", &graphics_ok(), DA1]);
        assert_eq!(parse_reply(&buf, ID).unwrap().graphics, GraphicsReply::Ok);
        // Alt-_ and Alt-P are not replies either.
        let buf = concat(&[b"\x1b_x\x1bPq", &graphics_ok(), DA1]);
        assert_eq!(parse_reply(&buf, ID).unwrap().graphics, GraphicsReply::Ok);
        let buf = concat(&[b"\x1b_G", &graphics_ok(), DA1]);
        assert_eq!(parse_reply(&buf, ID).unwrap().graphics, GraphicsReply::Ok);
    }

    #[test]
    fn report_strips_control_characters() {
        let detection = Detection {
            caps: GraphicsCaps::unsupported("terminal rejected the Kitty graphics query"),
            probe: Probe::Answered(ProbeReply {
                graphics: GraphicsReply::Error("EBAD\x07\x1b[2J".to_string()),
                version: Some("evil\x1b]0;x\x07".to_string()),
                cell_px: None,
            }),
        };
        let text = report(
            setting(GraphicsMode::Auto, SettingSource::Default),
            &detection,
        );
        assert!(!text.contains('\x1b') && !text.contains('\x07'), "{text:?}");
    }

    #[test]
    fn off_never_probes() {
        for source in [
            SettingSource::Cli,
            SettingSource::Env,
            SettingSource::Config,
        ] {
            let caps = decide_without_probe(
                setting(GraphicsMode::Off, source),
                OutputTarget::Pager,
                true,
                false,
            )
            .unwrap();
            assert!(!caps.supported);
        }
    }

    #[test]
    fn plain_or_non_tty_output_never_probes() {
        let auto = setting(GraphicsMode::Auto, SettingSource::Default);
        assert!(decide_without_probe(auto, OutputTarget::Plain, true, false).is_some());
        assert!(decide_without_probe(auto, OutputTarget::Pager, false, false).is_some());
    }

    #[test]
    fn tmux_resolves_off() {
        let auto = setting(GraphicsMode::Auto, SettingSource::Default);
        let caps = decide_without_probe(auto, OutputTarget::Pager, true, true).unwrap();
        assert!(!caps.supported);
        assert!(caps.via_tmux);
    }

    #[test]
    fn auto_pager_on_terminal_probes() {
        let auto = setting(GraphicsMode::Auto, SettingSource::Default);
        assert_eq!(
            decide_without_probe(auto, OutputTarget::Pager, true, false),
            None
        );
    }

    #[test]
    fn report_lists_decision_and_reason() {
        let detection = Detection {
            caps: GraphicsCaps::unsupported("disabled by --graphics off"),
            probe: Probe::NotSent,
        };
        let text = report(setting(GraphicsMode::Off, SettingSource::Cli), &detection);
        assert!(text.contains("graphics: off"), "{text}");
        assert!(
            text.contains("reason: disabled by --graphics off"),
            "{text}"
        );
        assert!(text.contains("setting: off (from --graphics)"), "{text}");
    }
}

#[cfg(all(test, unix))]
mod pty_tests {
    use super::sys::{exchange, wait_readable};
    use super::{GraphicsReply, PROBE_IMAGE_ID, query_bytes};
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::time::{Duration, Instant};

    /// A pty pair with the terminal side in raw mode: (terminal, emulator).
    fn raw_pty() -> (File, File) {
        let mut master = -1;
        let mut slave = -1;
        // SAFETY: both out-pointers are valid; name, termios and winsize may be null.
        let rc = unsafe {
            libc::openpty(
                &raw mut master,
                &raw mut slave,
                std::ptr::null_mut(),
                // Null pointers; macOS declares these `*mut`, Linux `*const`.
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, 0, "openpty failed");
        // SAFETY: openpty returned a fresh descriptor that nothing else owns.
        let tty = unsafe { File::from_raw_fd(slave) };
        // SAFETY: as above, for the other end.
        let emulator = unsafe { File::from_raw_fd(master) };
        // SAFETY: termios is plain data; tcgetattr fills it in below.
        let mut termios: libc::termios = unsafe { std::mem::zeroed() };
        // SAFETY: valid fd and termios pointer.
        let rc = unsafe { libc::tcgetattr(tty.as_raw_fd(), &raw mut termios) };
        assert_eq!(rc, 0);
        // SAFETY: termios was initialised by tcgetattr.
        unsafe { libc::cfmakeraw(&raw mut termios) };
        // SAFETY: valid fd and termios pointer.
        let rc = unsafe { libc::tcsetattr(tty.as_raw_fd(), libc::TCSANOW, &raw const termios) };
        assert_eq!(rc, 0);
        (tty, emulator)
    }

    /// Play the terminal: read the query up to DA1, wait `delay`, send `reply`.
    fn answer(
        mut emulator: File,
        delay: Duration,
        reply: &'static [u8],
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let mut seen = Vec::new();
            let mut chunk = [0u8; 256];
            while !seen.ends_with(b"\x1b[c") {
                let n = emulator.read(&mut chunk).expect("read query");
                seen.extend_from_slice(&chunk[..n]);
            }
            assert_eq!(seen, query_bytes(PROBE_IMAGE_ID));
            std::thread::sleep(delay);
            emulator.write_all(reply).expect("write reply");
            // Keep the emulator side open until the probe is done reading.
            std::thread::sleep(Duration::from_millis(500));
        })
    }

    const KITTY: &[u8] = b"\x1b_Gi=28004;OK\x1b\\\x1bP>|kitty(0.35.2)\x1b\\\x1b[?62;22c";

    #[test]
    fn slow_reply_is_read_to_the_end() {
        let (mut tty, emulator) = raw_pty();
        let terminal = answer(emulator, Duration::from_millis(300), KITTY);
        let reply = exchange(
            &mut tty,
            &query_bytes(PROBE_IMAGE_ID),
            Duration::from_secs(2),
        )
        .unwrap()
        .expect("reply within the timeout");
        assert_eq!(reply.graphics, GraphicsReply::Ok);
        assert_eq!(reply.version.as_deref(), Some("kitty(0.35.2)"));
        terminal.join().unwrap();
    }

    #[test]
    fn timeout_drops_partial_reply() {
        let (mut tty, emulator) = raw_pty();
        let terminal = answer(emulator, Duration::ZERO, b"\x1b_Gi=28004;OK\x1b\\Gi");
        let start = Instant::now();
        let reply = exchange(
            &mut tty,
            &query_bytes(PROBE_IMAGE_ID),
            Duration::from_millis(200),
        );
        assert!(reply.unwrap().is_none());
        assert!(start.elapsed() >= Duration::from_millis(200));
        // The partial reply was consumed, so nothing is left for the pager to
        // read as keystrokes. (Bytes arriving between the last read and the
        // flush cannot be staged deterministically, so the flush itself is
        // not exercised here.)
        assert!(!wait_readable(&tty, Instant::now() + Duration::from_millis(50)).unwrap());
        terminal.join().unwrap();
    }

    #[test]
    fn silent_terminal_times_out() {
        let (mut tty, _emulator) = raw_pty();
        let start = Instant::now();
        let reply = exchange(
            &mut tty,
            &query_bytes(PROBE_IMAGE_ID),
            Duration::from_millis(100),
        );
        assert!(reply.unwrap().is_none());
        assert!(start.elapsed() < Duration::from_secs(1));
    }
}
