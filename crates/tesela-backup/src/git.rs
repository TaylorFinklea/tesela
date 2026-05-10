//! Git remote destination: maintain a local mirror repo + push each
//! backup as a commit.
//!
//! We shell out to `git` rather than link libgit2. Reasons: the user
//! clearly has git installed (this codebase lives in one), we avoid a
//! heavyweight native dep, and credentialing (SSH keys, GitHub PATs,
//! macOS Keychain helper) is handled by the system's git invocations
//! exactly the way the user already has it configured.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{BackupError, Result};

/// Ensure the local mirror is a git repo on the right branch, with the
/// configured remote present and (optionally) populated from a fresh
/// `git fetch`. Safe to call repeatedly.
pub fn ensure_mirror(mirror: &Path, remote: &str, branch: &str) -> Result<()> {
    std::fs::create_dir_all(mirror)?;
    if !mirror.join(".git").exists() {
        run_git(mirror, &["init", "--quiet", "--initial-branch", branch])?;
        // Identity is set per-repo so global gitconfig changes don't
        // affect our commits and vice versa.
        run_git(mirror, &["config", "user.email", "tesela-backup@localhost"])?;
        run_git(mirror, &["config", "user.name", "Tesela Backup"])?;
    }
    // Idempotently add/update the origin remote.
    let remote_status = Command::new("git")
        .arg("-C")
        .arg(mirror)
        .args(["remote", "get-url", "origin"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if remote_status.success() {
        run_git(mirror, &["remote", "set-url", "origin", remote])?;
    } else {
        run_git(mirror, &["remote", "add", "origin", remote])?;
    }
    Ok(())
}

/// Stage everything, commit with `message`, then push to `origin/branch`.
pub fn commit_and_push(mirror: &Path, branch: &str, message: &str) -> Result<()> {
    run_git(mirror, &["add", "-A"])?;
    // `git commit` exits non-zero if nothing changed; treat that as
    // OK so retention-only runs don't crash.
    let commit_status = Command::new("git")
        .arg("-C")
        .arg(mirror)
        .args(["commit", "-m", message, "--allow-empty"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !commit_status.success() {
        // --allow-empty means this should always succeed; if it
        // doesn't, surface the underlying error rather than silently
        // proceeding.
        let captured = Command::new("git")
            .arg("-C")
            .arg(mirror)
            .args(["commit", "-m", message, "--allow-empty"])
            .output()?;
        return Err(BackupError::Other(anyhow::anyhow!(
            "git commit failed: {}",
            String::from_utf8_lossy(&captured.stderr).trim()
        )));
    }
    run_git(mirror, &["push", "-u", "origin", branch])?;
    Ok(())
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git").arg("-C").arg(cwd).args(args).output()?;
    if !output.status.success() {
        return Err(BackupError::Other(anyhow::anyhow!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}
