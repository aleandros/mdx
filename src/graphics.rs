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

/// How long to wait for the terminal's replies on a local terminal.
const PROBE_TIMEOUT: Duration = Duration::from_millis(100);

/// Over SSH the round trip can exceed [`PROBE_TIMEOUT`]; a reply arriving
/// after the timeout would otherwise be read by the pager as keystrokes.
const PROBE_TIMEOUT_SSH: Duration = Duration::from_millis(500);

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
/// config file, then `auto`. An empty environment value counts as unset.
pub(crate) fn resolve_setting(
    cli: Option<GraphicsMode>,
    env: Option<&str>,
    config: Option<GraphicsMode>,
) -> anyhow::Result<GraphicsSetting> {
    if let Some(mode) = cli {
        return Ok(GraphicsSetting {
            mode,
            source: SettingSource::Cli,
        });
    }
    if let Some(value) = env.filter(|v| !v.trim().is_empty()) {
        let mode = parse_mode(value).ok_or_else(|| {
            anyhow::anyhow!("invalid {ENV_VAR} value {value:?} (expected \"auto\" or \"off\")")
        })?;
        return Ok(GraphicsSetting {
            mode,
            source: SettingSource::Env,
        });
    }
    Ok(config.map_or(
        GraphicsSetting {
            mode: GraphicsMode::Auto,
            source: SettingSource::Default,
        },
        |mode| GraphicsSetting {
            mode,
            source: SettingSource::Config,
        },
    ))
}

/// Resolve the setting from the CLI flag, the process environment and the config.
pub(crate) fn setting_from_env(
    cli: Option<GraphicsMode>,
    config: Option<GraphicsMode>,
) -> anyhow::Result<GraphicsSetting> {
    let env = std::env::var(ENV_VAR).ok();
    resolve_setting(cli, env.as_deref(), config)
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

fn find_st(buf: &[u8], from: usize) -> Option<usize> {
    buf.get(from..)?
        .windows(2)
        .position(|w| w == [ESC, b'\\'])
        .map(|p| from + p)
}

/// Parse the replies read so far. Returns `None` until the DA1 reply has
/// arrived; anything after it is not part of the reply. Unrelated bytes are
/// skipped.
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
            b'_' | b'P' => {
                let end = find_st(buf, i + 2)?;
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
                let len = buf
                    .get(start..)?
                    .iter()
                    .position(|b| (0x40..=0x7e).contains(b))?;
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
            GraphicsReply::Error(message) => format!("error: {message}"),
        },
    };
    let terminal = match &detection.probe {
        Probe::Answered(ProbeReply {
            version: Some(version),
            ..
        }) => version.as_str(),
        _ => "unknown",
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

    /// Send `query` to the controlling terminal and read replies until DA1
    /// or `timeout`. Returns the parsed reply (`None` on timeout) and the
    /// cell size from `TIOCGWINSZ`.
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

        tty.write_all(query)?;
        tty.flush()?;

        let deadline = Instant::now() + timeout;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 256];
        loop {
            if let Some(reply) = super::parse_reply(&buf, super::PROBE_IMAGE_ID) {
                return Ok((Some(reply), winsize_cell));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() || !wait_readable(&tty, remaining)? {
                break;
            }
            let n = tty.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        // Drop a partial reply so the pager does not read it as keystrokes.
        // SAFETY: tcflush only discards queued input on a valid, open fd.
        unsafe {
            libc::tcflush(tty.as_raw_fd(), libc::TCIFLUSH);
        }
        Ok((None, winsize_cell))
    }

    /// Wait until `tty` has input or `timeout` passes. Uses `select` because
    /// macOS `poll` does not support tty devices.
    fn wait_readable(tty: &File, timeout: Duration) -> std::io::Result<bool> {
        let fd = tty.as_raw_fd();
        let in_range = usize::try_from(fd).is_ok_and(|fd| fd < libc::FD_SETSIZE);
        if !in_range {
            return Err(std::io::Error::other("tty descriptor too large for select"));
        }
        loop {
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
        )
        .unwrap();
        assert_eq!(s, setting(GraphicsMode::Auto, SettingSource::Cli));
    }

    #[test]
    fn precedence_env_over_config() {
        let s = resolve_setting(None, Some("OFF"), Some(GraphicsMode::Auto)).unwrap();
        assert_eq!(s, setting(GraphicsMode::Off, SettingSource::Env));
    }

    #[test]
    fn precedence_config_then_default() {
        let s = resolve_setting(None, None, Some(GraphicsMode::Off)).unwrap();
        assert_eq!(s, setting(GraphicsMode::Off, SettingSource::Config));
        let s = resolve_setting(None, Some(""), None).unwrap();
        assert_eq!(s, setting(GraphicsMode::Auto, SettingSource::Default));
    }

    #[test]
    fn invalid_env_value_errors() {
        let err = resolve_setting(None, Some("on"), None).unwrap_err();
        assert!(err.to_string().contains("MDX_GRAPHICS"), "{err}");
        // The CLI flag makes the environment irrelevant.
        assert!(resolve_setting(Some(GraphicsMode::Off), Some("on"), None).is_ok());
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
