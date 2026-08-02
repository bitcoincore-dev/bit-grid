use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::{io, time::Duration};

struct App {
    /// 256 bits represented as 32 bytes (256 bits total)
    bits: [u8; 32],
    /// Current selected index in the 256-bit array (0..255)
    cursor: usize,
    /// Whether the app should quit
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            bits: [0u8; 32],
            cursor: 0,
            should_quit: false,
        }
    }

    fn toggle_bit(&mut self) {
        let byte_idx = self.cursor / 8;
        let bit_idx = 7 - (self.cursor % 8); // MSB left, LSB right
        self.bits[byte_idx] ^= 1 << bit_idx;
    }

    fn randomize_bits(&mut self) {
        for byte in self.bits.iter_mut() {
            *byte = rand_simple();
        }
    }

    fn clear_bits(&mut self) {
        self.bits = [0u8; 32];
    }

    fn move_cursor(&mut self, dx: i32, dy: i32) {
        let col = (self.cursor % 16) as i32;
        let row = (self.cursor / 16) as i32;

        let new_col = (col + dx).rem_euclid(16);
        let new_row = (row + dy).rem_euclid(16);

        self.cursor = (new_row * 16 + new_col) as usize;
    }
}

/// Simple LCG randomizer to avoid adding additional crates
fn rand_simple() -> u8 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEED: AtomicU32 = AtomicU32::new(0xDEAD_BEEF);
    let old = SEED.fetch_add(0x4519_3571, Ordering::Relaxed);
    let state = old.wrapping_mul(1103515245).wrapping_add(12345);
    (state >> 16) as u8
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    // Event loop
    while !app.should_quit {
        terminal.draw(|f| ui(f, &mut app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                        KeyCode::Char(' ') => app.toggle_bit(),
                        KeyCode::Char('r') => app.randomize_bits(),
                        KeyCode::Char('c') => app.clear_bits(),
                        KeyCode::Left | KeyCode::Char('h') => app.move_cursor(-1, 0),
                        KeyCode::Right | KeyCode::Char('l') => app.move_cursor(1, 0),
                        KeyCode::Up | KeyCode::Char('k') => app.move_cursor(0, -1),
                        KeyCode::Down | KeyCode::Char('j') => app.move_cursor(0, 1),
                        _ => {}
                    }
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Header / Info
            Constraint::Min(18),   // 16x16 Grid Box
            Constraint::Length(4), // Hex/Byte View & Help
        ])
        .split(f.size());

    // 1. Header Area
    let byte_idx = app.cursor / 8;
    let bit_idx = 7 - (app.cursor % 8);
    let header_text = format!(
        " 256-BIT GRID INSPECTOR | Selected Bit: {} (Byte {}, Bit {}) ",
        app.cursor, byte_idx, bit_idx
    );
    let header = Paragraph::new(header_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL).title(" Overview "));
    f.render_widget(header, chunks[0]);

    // 2. 16x16 Grid Construction
    let grid_block = Block::default().borders(Borders::ALL).title(" 16x16 Bit Grid (256 Bits) ");
    let inner_grid_area = grid_block.inner(chunks[1]);
    f.render_widget(grid_block, chunks[1]);

    let mut grid_lines = Vec::new();
    for row in 0..16 {
        let mut spans = Vec::new();
        spans.push(Span::styled(
            format!("{:02X}0 | ", row),
            Style::default().fg(Color::DarkGray),
        ));

        for col in 0..16 {
            let bit_flat_idx = row * 16 + col;
            let b_idx = bit_flat_idx / 8;
            let b_shift = 7 - (bit_flat_idx % 8);
            let is_set = (app.bits[b_idx] & (1 << b_shift)) != 0;
            let is_cursor = bit_flat_idx == app.cursor;

            let symbol = if is_set { "█ " } else { "· " };

            let style = match (is_cursor, is_set) {
                (true, true) => Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
                (true, false) => Style::default()
                    .fg(Color::Gray)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
                (false, true) => Style::default().fg(Color::Green),
                (false, false) => Style::default().fg(Color::DarkGray),
            };

            spans.push(Span::styled(symbol, style));
        }
        grid_lines.push(Line::from(spans));
    }

    let grid_paragraph = Paragraph::new(grid_lines).alignment(Alignment::Center);
    f.render_widget(grid_paragraph, inner_grid_area);

    // 3. Hex Output & Controls
    let hex_str: String = app.bits.iter().map(|b| format!("{:02x}", b)).collect();
    let controls = " [Arrow Keys/Vim] Move | [Space] Toggle | [r] Randomize | [c] Clear | [q] Quit ";
    
    let footer_lines = vec![
        Line::from(vec![
            Span::styled("HEX: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(hex_str, Style::default().fg(Color::White)),
        ]),
        Line::from(Span::styled(controls, Style::default().fg(Color::DarkGray))),
    ];

    let footer = Paragraph::new(footer_lines)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" State & Keybindings "));
    f.render_widget(footer, chunks[2]);
}

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
