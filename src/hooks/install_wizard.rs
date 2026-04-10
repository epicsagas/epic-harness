//! Full-screen install picker: arrow keys, Space toggles, A / N for all / none.

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, queue};
use std::io::{self, Write};

const ACCENT: crossterm::style::Color = crossterm::style::Color::Cyan;
const DIM: crossterm::style::Color = crossterm::style::Color::DarkGrey;
const HI: crossterm::style::Color = crossterm::style::Color::Yellow;

/// Interactive multi-select when stdin+stderr are TTYs. On error or non-interactive use, caller may fall back.
pub fn interactive_select_tools(tools: &[(&str, &str)]) -> io::Result<Vec<String>> {
    run_tui(tools)
}

fn selected_names(tools: &[(&str, &str)], checked: &[bool]) -> Vec<String> {
    tools
        .iter()
        .zip(checked.iter())
        .filter_map(|((name, _), &on)| on.then_some(name.to_string()))
        .collect()
}

fn run_tui(tools: &[(&str, &str)]) -> io::Result<Vec<String>> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        Hide,
        MoveTo(0, 0),
        Clear(ClearType::All)
    )?;

    struct RawGuard;
    impl Drop for RawGuard {
        fn drop(&mut self) {
            let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
            let _ = disable_raw_mode();
        }
    }
    let _guard = RawGuard;

    let mut checked = vec![false; tools.len()];
    let mut cursor = 0usize;

    loop {
        draw(&mut stdout, tools, &checked, cursor)?;
        stdout.flush()?;

        let ev = event::read()?;
        let Event::Key(key) = ev else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cursor = cursor.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                if cursor + 1 < tools.len() {
                    cursor += 1;
                }
            }
            KeyCode::Char(' ') => {
                checked[cursor] = !checked[cursor];
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                let all_on = checked.iter().all(|&c| c);
                let set = !all_on;
                checked.fill(set);
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                checked.fill(false);
            }
            KeyCode::Enter => {
                return Ok(selected_names(tools, &checked));
            }
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                return Ok(vec![]);
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(vec![]);
            }
            _ => {}
        }
    }
}

fn draw(
    w: &mut impl Write,
    tools: &[(&str, &str)],
    checked: &[bool],
    cursor: usize,
) -> io::Result<()> {
    queue!(w, MoveTo(0, 0), Clear(ClearType::All))?;

    queue!(
        w,
        SetForegroundColor(ACCENT),
        SetAttribute(Attribute::Bold),
        Print(" ╭────────────────────────────────────────────────────────────────────────╮\r\n"),
        Print(" │ "),
        ResetColor,
        SetForegroundColor(ACCENT),
        SetAttribute(Attribute::Bold),
        Print("epic-harness"),
        ResetColor,
        Print("  ·  Install integrations"),
        SetForegroundColor(ACCENT),
        Print("                                    │\r\n"),
        Print(" ╰────────────────────────────────────────────────────────────────────────╯\r\n"),
        ResetColor,
        Print("\r\n"),
    )?;

    for (i, ((name, desc), on)) in tools.iter().zip(checked.iter()).enumerate() {
        let row_hi = i == cursor;
        let mark = if *on { "[x]" } else { "[ ]" };
        let prefix = if row_hi { " › " } else { "   " };

        if row_hi {
            queue!(
                w,
                SetForegroundColor(HI),
                SetAttribute(Attribute::Bold),
                Print(prefix),
                Print(mark),
                Print("  "),
                Print(format!("{name:<12}")),
                ResetColor,
                SetForegroundColor(DIM),
                Print("  "),
                Print(truncate_desc(desc, 44)),
                ResetColor,
                Print("\r\n"),
            )?;
        } else {
            queue!(
                w,
                Print(prefix),
                SetForegroundColor(DIM),
                Print(mark),
                ResetColor,
                Print("  "),
                Print(format!("{name:<12}")),
                SetForegroundColor(DIM),
                Print("  "),
                Print(truncate_desc(desc, 44)),
                ResetColor,
                Print("\r\n"),
            )?;
        }
    }

    queue!(
        w,
        Print("\r\n"),
        SetForegroundColor(DIM),
        Print(" ────────────────────────────────────────────────────────────────────────\r\n"),
        Print("  ↑/↓ Move   Space Toggle   A All / none   N Clear   Enter Confirm   Esc Q Quit\r\n"),
        ResetColor,
    )?;

    Ok(())
}

fn truncate_desc(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let take = max_chars.saturating_sub(1);
    s.chars().take(take).chain(std::iter::once('…')).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_names_filters_checked() {
        let tools = &[("a", "d1"), ("b", "d2")];
        let checked = &[true, false];
        assert_eq!(selected_names(tools, checked), vec!["a".to_string()]);
    }
}
