//! Flash for Rex: herdr-flash's picker over one Rex block.
//!
//! Rex can float a terminal exactly over a block, so this runs inside that float and paints its
//! own terminal; the Herdr plugin needs a hidden layout tab, launch barriers, and placeholder
//! ttys for the same effect. The float runs on the block's host, so `rex` here talks to the
//! server that holds the block.

use anyhow::{bail, Context, Result};
use clap::Parser;
use herdr_flash::config::{resolve_flash_exit_on_yank, resolve_palette};
use herdr_flash::model::{
    PaneId, PaneTextCaptureMode, PickerAction, PickerPaneSnapshot, PickerReturnContext,
    PickerSnapshot, SourcePaneSnapshot,
};
use herdr_flash::viewport::map_visible_viewport;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

/// Search the visible text of a Rex block, select, and yank over OSC 52.
#[derive(Debug, Parser)]
#[command(name = "rex-flash", version)]
struct Cli {
    /// The Rex block to search.
    #[arg(long)]
    block: String,
    /// The block's width in cells.
    #[arg(long)]
    cols: u16,
    /// The block's height in cells.
    #[arg(long)]
    rows: u16,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    // herdr-flash reads its config from the directory Herdr names; Rex has its own.
    if std::env::var_os("HERDR_PLUGIN_CONFIG_DIR").is_none() {
        std::env::set_var("HERDR_PLUGIN_CONFIG_DIR", config_dir());
    }

    let output = Command::new("rex")
        .args(["capture", "-b", &cli.block])
        .output()
        .context("failed to run rex capture")?;
    if !output.status.success() {
        bail!(
            "rex capture failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let rows = visible_rows(&String::from_utf8_lossy(&output.stdout), cli.rows);
    let snapshot = flash_snapshot(&cli.block, rows, cli.cols, cli.rows);

    // A new float starts at a default size; Rex resizes it to its bounds once the app shows it.
    // Painting before that would misalign the frame with the block underneath.
    if let Err(error) = wait_for_size(cli.cols, cli.rows, Duration::from_secs(1)) {
        eprintln!("rex-flash: {error:#}");
    }
    herdr_flash::picker::run_flash_picker(&snapshot, &mut []).map(|_| ())
}

/// `$XDG_CONFIG_HOME/rex-flash`, else `~/.config/rex-flash`. A missing file means defaults.
fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_default()
        .join("rex-flash")
}

fn wait_for_size(width: u16, height: u16, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    loop {
        let size = crossterm::terminal::size().context("failed to read terminal size")?;
        if size == (width, height) {
            return Ok(());
        }
        if started.elapsed() >= timeout {
            bail!(
                "terminal is {}x{}, not the block's {width}x{height}",
                size.0,
                size.1
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The screen is the last `height` lines of a capture; the rest is scrollback.
fn visible_rows(capture: &str, height: u16) -> Vec<String> {
    let capture = capture.strip_suffix('\n').unwrap_or(capture);
    let lines: Vec<&str> = capture.split('\n').collect();
    lines[lines.len().saturating_sub(usize::from(height))..]
        .iter()
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect()
}

fn flash_snapshot(block: &str, rows: Vec<String>, width: u16, height: u16) -> PickerSnapshot {
    let viewport = map_visible_viewport(rows, width, height);
    PickerSnapshot {
        source: SourcePaneSnapshot {
            target_pane_id: PaneId::new(block),
            source_tab_id: String::new(),
            workspace_id: String::new(),
            source_panes: Vec::new(),
            target_content_width: width,
            target_content_height: height,
            logical_lines: viewport.logical_lines.clone(),
            visible_viewport: Some(viewport),
            capture_mode: PaneTextCaptureMode::VisibleWrapped,
            siblings: Vec::new(),
        },
        picker: PickerPaneSnapshot {
            content_width: width,
            content_height: height,
        },
        session: PickerReturnContext {
            return_tab_id: String::new(),
            return_pane_id: PaneId::new(block),
        },
        action: PickerAction::Flash,
        custom_patterns: Vec::new(),
        flash_exit_on_yank: resolve_flash_exit_on_yank(),
        palette: resolve_palette(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_rows_drop_scrollback_and_keep_blank_screen_rows() {
        let capture = "old\nscreen 1\n\nscreen 3\n";
        assert_eq!(visible_rows(capture, 3), vec!["screen 1", "", "screen 3"]);
    }

    #[test]
    fn short_capture_is_padded_to_the_block_height() {
        let snapshot = flash_snapshot("block:1", visible_rows("only\n", 3), 10, 3);
        let viewport = snapshot.source.visible_viewport.unwrap();
        assert_eq!(viewport.rows, vec!["only", "", ""]);
        assert_eq!(snapshot.source.logical_lines, viewport.logical_lines);
    }
}
