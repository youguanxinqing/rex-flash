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
use std::num::NonZeroU16;
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
    /// The block's text, if the caller already has it. Without it, rex-flash runs `rex capture`,
    /// which costs 20-40 ms while the float sits blank.
    #[arg(long)]
    screen: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    // herdr-flash reads its config from the directory Herdr names; Rex has its own.
    if std::env::var_os("HERDR_PLUGIN_CONFIG_DIR").is_none() {
        std::env::set_var("HERDR_PLUGIN_CONFIG_DIR", config_dir());
    }

    let screen = match cli.screen {
        Some(screen) => screen,
        None => capture(&cli.block)?,
    };
    let rows = visible_rows(&screen, cli.cols, cli.rows);
    let snapshot = flash_snapshot(&cli.block, rows, cli.cols, cli.rows);

    // A new float starts at a default size; Rex resizes it to its bounds once the app shows it.
    // Painting before that would misalign the frame with the block underneath.
    if let Err(error) = wait_for_size(cli.cols, cli.rows, Duration::from_secs(1)) {
        eprintln!("rex-flash: {error:#}");
    }
    herdr_flash::picker::run_flash_picker(&snapshot, &mut []).map(|_| ())
}

fn capture(block: &str) -> Result<String> {
    let output = Command::new("rex")
        // vt, not text: text drops the blank rows at the bottom of the screen, so its last lines
        // reach up into the scrollback.
        .args(["capture", "-b", block, "--format=vt"])
        .output()
        .context("failed to run rex capture")?;
    if !output.status.success() {
        bail!(
            "rex capture failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
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

/// The block's screen, from a vt capture replayed through a terminal emulator at the block's size.
/// vt is Rex's redraw stream, so any escape it uses, such as cursor-forward over a run of blank
/// cells, lands where the block shows it, and the scrollback scrolls off the top.
fn visible_rows(capture: &str, width: u16, height: u16) -> Vec<String> {
    // vt ends the last row with nothing, so a trailing \r\n is an empty last row. `rex capture`
    // appends a bare \n when the last row isn't empty; replaying it would scroll the screen.
    let capture = match capture.strip_suffix('\n') {
        Some(rest) if !rest.ends_with('\r') => rest,
        _ => capture,
    };
    let size = |n: u16| NonZeroU16::new(n).unwrap_or(NonZeroU16::MIN);
    let mut parser = vt100::Parser::new(size(height), size(width), 0);
    parser.process(capture.as_bytes());
    parser.screen().rows(0, width).collect()
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
    fn blank_rows_at_the_bottom_of_the_screen_stay_on_screen() {
        // Two scrollback lines, then a 4-row screen whose last two rows are blank.
        let vt = "\x1b[0mold 1\r\nold 2\r\n\x1b[38;5;1mtop\x1b[0m\r\n$\r\n\r\n";
        assert_eq!(visible_rows(vt, 10, 4), vec!["top", "$", "", ""]);
        // rex capture appends a bare newline when the last row isn't empty.
        assert_eq!(visible_rows("old\r\na\r\nb\n", 10, 2), vec!["a", "b"]);
    }

    #[test]
    fn escapes_take_their_cells() {
        // Cursor-forward skips blank cells (a right prompt), a hyperlink wraps text, and a row
        // exactly the block's width doesn't wrap into the next.
        let vt = "$ ls\x1b[3C<<<\r\n\x1b]8;;http://x\x1b\\link\x1b]8;;\x1b\\\r\n0123456789\r\nnext";
        assert_eq!(
            visible_rows(vt, 10, 4),
            vec!["$ ls   <<<", "link", "0123456789", "next"]
        );
    }

    #[test]
    fn short_capture_is_padded_to_the_block_height() {
        let snapshot = flash_snapshot("block:1", visible_rows("only\n", 10, 3), 10, 3);
        let viewport = snapshot.source.visible_viewport.unwrap();
        assert_eq!(viewport.rows, vec!["only", "", ""]);
        assert_eq!(snapshot.source.logical_lines, viewport.logical_lines);
    }
}
