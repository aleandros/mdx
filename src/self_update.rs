use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

const REPO: &str = "aleandros/mdx";

pub fn run() -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");

    println!("Checking for updates...");
    let latest_tag = fetch_latest_tag()?;
    let latest_version = latest_tag.trim_start_matches('v');

    if current_version == latest_version {
        println!("Already up to date (v{current_version})");
        return Ok(());
    }

    println!("Updating mdx v{current_version} → v{latest_version}...");

    let target = detect_target()?;
    let url =
        format!("https://github.com/{REPO}/releases/download/{latest_tag}/mdx-{target}.tar.gz");

    let tmp_dir = std::env::temp_dir().join(format!("mdx-update-{}", std::process::id()));
    std::fs::create_dir_all(&tmp_dir).context("Failed to create temporary directory")?;
    let _cleanup = CleanupGuard(&tmp_dir);

    download_and_extract(&url, &tmp_dir).context("Failed to download update")?;

    let new_binary = tmp_dir.join("mdx");
    if !new_binary.exists() {
        anyhow::bail!("Downloaded archive did not contain mdx binary");
    }

    let current_exe =
        std::env::current_exe().context("Failed to determine current executable path")?;

    replace_binary(&new_binary, &current_exe)?;

    println!("Updated mdx to v{latest_version}");
    Ok(())
}

fn fetch_latest_tag() -> Result<String> {
    let output = Command::new("curl")
        .args([
            "-fsSL",
            &format!("https://api.github.com/repos/{REPO}/releases/latest"),
        ])
        .output()
        .context("Failed to fetch release info (is curl installed?)")?;

    if !output.status.success() {
        anyhow::bail!("Failed to fetch latest release from GitHub");
    }

    let body = String::from_utf8(output.stdout).context("Invalid UTF-8 in GitHub API response")?;

    parse_tag_from_json(&body).context("Could not find tag_name in GitHub API response")
}

fn parse_tag_from_json(body: &str) -> Option<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"tag_name\"") {
            // Format: "tag_name": "v0.1.3",
            let value = trimmed.split(':').nth(1)?;
            let tag = value
                .trim()
                .trim_matches(|c: char| c == '"' || c == ',' || c.is_whitespace());
            if !tag.is_empty() {
                return Some(tag.to_string());
            }
        }
    }
    None
}

fn detect_target() -> Result<String> {
    let os = match std::env::consts::OS {
        "linux" => "unknown-linux-gnu",
        "macos" => "apple-darwin",
        other => anyhow::bail!("Unsupported OS: {other}"),
    };

    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => anyhow::bail!("Unsupported architecture: {other}"),
    };

    Ok(format!("{arch}-{os}"))
}

fn download_and_extract(url: &str, dest: &Path) -> Result<()> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "curl -fsSL '{}' | tar xz -C '{}'",
            url,
            dest.display()
        ))
        .status()
        .context("Failed to run curl | tar")?;

    if !status.success() {
        anyhow::bail!("Download failed (HTTP error or invalid archive)");
    }

    Ok(())
}

fn replace_binary(new_binary: &Path, current_exe: &Path) -> Result<()> {
    // On Unix, we can unlink a running binary — the process keeps the open
    // file descriptor and the inode lives until all references close.
    // Then we copy the new binary into the same path.
    std::fs::remove_file(current_exe).with_context(|| {
        format!(
            "Cannot replace {} (try: sudo mdx update)",
            current_exe.display()
        )
    })?;

    std::fs::copy(new_binary, current_exe)
        .with_context(|| format!("Failed to install new binary to {}", current_exe.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(current_exe, std::fs::Permissions::from_mode(0o755))
            .context("Failed to set executable permissions")?;
    }

    Ok(())
}

struct CleanupGuard<'a>(&'a Path);

impl Drop for CleanupGuard<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tag_from_json() {
        let json = r#"{
  "tag_name": "v0.2.0",
  "name": "v0.2.0",
  "draft": false
}"#;
        assert_eq!(parse_tag_from_json(json), Some("v0.2.0".to_string()));
    }

    #[test]
    fn test_parse_tag_missing() {
        assert_eq!(parse_tag_from_json("{}"), None);
        assert_eq!(parse_tag_from_json(""), None);
    }

    #[test]
    fn test_detect_target() {
        // Should succeed on Linux/macOS with x86_64/aarch64
        let target = detect_target().unwrap();
        assert!(
            target.ends_with("-unknown-linux-gnu") || target.ends_with("-apple-darwin"),
            "unexpected target: {target}"
        );
        assert!(
            target.starts_with("x86_64-") || target.starts_with("aarch64-"),
            "unexpected target: {target}"
        );
    }
}
