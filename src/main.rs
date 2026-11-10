use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use frizbee::{Config, Matcher};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Parser)]
#[command(version, about = "Quickly jump to a directory with fuzzy search")]
struct Args {
    /// Directory tree to search (defaults to your home directory)
    #[arg(short, long)]
    path: Option<PathBuf>,

    /// Maximum directory depth to scan
    #[arg(short, long, default_value_t = 8)]
    depth: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let root = args
        .path
        .or_else(dirs::home_dir)
        .or_else(|| std::env::current_dir().ok())
        .context("could not determine a starting directory")?;
    let root = root.canonicalize().with_context(|| {
        format!("starting directory does not exist: {}", root.display())
    })?;

    let directories = find_directories(&root, args.depth);
    if directories.is_empty() {
        anyhow::bail!("no directories found under {}", root.display());
    }

    // Print only the selected directory to stdout so this command can be used
    // inside a shell function such as: cd "$(navi-rust-cli)".
    if let Some(selected) = run_picker(&directories)? {
        println!("{}", selected.display());
    }
    Ok(())
}

fn find_directories(root: &Path, depth: usize) -> Vec<PathBuf> {
    WalkDir::new(root)
        .min_depth(1)
        .max_depth(depth.saturating_add(1))
        .follow_links(false)
        .into_iter()
        .filter_entry(should_visit)
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .map(DirEntry::into_path)
        .collect()
}

fn should_visit(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !name.starts_with('.') && !matches!(name.as_ref(), "node_modules" | "target")
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen, Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(error.into());
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn run_picker(directories: &[PathBuf]) -> Result<Option<PathBuf>> {
    let _guard = TerminalGuard::enter()?;
    let labels: Vec<String> = directories
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    let mut matcher = Matcher::from_query("", &Config::default());
    let mut query = String::new();
    let mut selected = 0usize;

    loop {
        let matches = if query.is_empty() {
            Vec::new()
        } else {
            matcher.match_list(&labels)
        };
        let count = if query.is_empty() { labels.len() } else { matches.len() };
        if count == 0 {
            selected = 0;
        } else if selected >= count {
            selected = count - 1;
        }
        draw(&query, &labels, &matches, selected)?;

        if event::poll(Duration::from_millis(100))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc | KeyCode::Char('c') if key.code == KeyCode::Esc => {
                    return Ok(None);
                }
                KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    return Ok(None);
                }
                KeyCode::Enter => {
                    let index = if query.is_empty() {
                        Some(selected)
                    } else {
                        matches.get(selected).map(|item| item.index as usize)
                    };
                    return Ok(index.and_then(|index| directories.get(index).cloned()));
                }
                KeyCode::Up | KeyCode::Char('k') if key.code == KeyCode::Up => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') if key.code == KeyCode::Down => {
                    if count > 0 {
                        selected = (selected + 1).min(count - 1);
                    }
                }
                KeyCode::PageUp => selected = selected.saturating_sub(10),
                KeyCode::PageDown => selected = (selected + 10).min(count.saturating_sub(1)),
                KeyCode::Backspace => {
                    query.pop();
                    selected = 0;
                }
                KeyCode::Char(ch) if !key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    query.push(ch);
                    selected = 0;
                }
                _ => {}
            }
        }
    }
}

fn draw(query: &str, labels: &[String], matches: &[frizbee::Match], selected: usize) -> Result<()> {
    let (width, height) = terminal::size()?;
    let visible_rows = height.saturating_sub(3) as usize;
    let mut stdout = io::stdout();
    queue!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
    queue!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print("navi  "),
        ResetColor,
        Print(format!("{query}_")),
        MoveTo(0, 1),
        Print("↑/↓ move  Enter select  Esc quit  Backspace edit"),
        MoveTo(0, 2),
    )?;

    let count = if query.is_empty() { labels.len() } else { matches.len() };
    let start = selected.saturating_sub(visible_rows.saturating_sub(1));
    let end = count.min(start.saturating_add(visible_rows));
    for row in start..end {
        let label = if query.is_empty() {
            labels.get(row)
        } else {
            matches.get(row).and_then(|item| labels.get(item.index as usize))
        };
        if let Some(label) = label {
            queue!(stdout, MoveTo(0, (row - start + 2) as u16))?;
            if row == selected {
                queue!(stdout, SetForegroundColor(Color::Black))?;
                queue!(stdout, crossterm::style::SetBackgroundColor(Color::Cyan))?;
            }
            let clipped: String = label.chars().take(width as usize).collect();
            queue!(stdout, Print(clipped), ResetColor)?;
        }
    }
    if count == 0 {
        queue!(stdout, MoveTo(0, 2), Print("No matching directories"))?;
    }
    stdout.flush()?;
    Ok(())
}
