use bitcoin::secp256k1::SecretKey;
use bitcoin::{
    Address, Network,
    address::NetworkUnchecked,
    key::{CompressedPublicKey, PrivateKey, Secp256k1},
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use rand::{RngCore, rngs::OsRng};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use std::{error::Error, fmt, io, time::Duration};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DemoNetwork {
    Testnet,
    Testnet4,
    Signet,
    Regtest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BitArt {
    Checker,
    Border,
    X,
    Diamond,
    Plus,
    Diagonal,
}

impl BitArt {
    const ALL: [Self; 6] = [
        Self::Checker,
        Self::Border,
        Self::X,
        Self::Diamond,
        Self::Plus,
        Self::Diagonal,
    ];

    fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|art| *art == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }
}

impl fmt::Display for BitArt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Checker => "checker",
            Self::Border => "border",
            Self::X => "x",
            Self::Diamond => "diamond",
            Self::Plus => "plus",
            Self::Diagonal => "diagonal",
        })
    }
}

impl DemoNetwork {
    const ALL: [Self; 4] = [Self::Testnet, Self::Testnet4, Self::Signet, Self::Regtest];

    fn next(self) -> Self {
        let idx = Self::ALL
            .iter()
            .position(|network| *network == self)
            .unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    fn as_bitcoin_network(self) -> Network {
        match self {
            Self::Testnet => Network::Testnet,
            Self::Testnet4 => Network::Testnet4,
            Self::Signet => Network::Signet,
            Self::Regtest => Network::Regtest,
        }
    }

    fn bech32_hrp(self) -> &'static str {
        match self {
            Self::Regtest => "bcrt",
            Self::Testnet | Self::Testnet4 | Self::Signet => "tb",
        }
    }

    fn wif_prefix(self) -> &'static str {
        let _ = self;
        "0xef"
    }
}

impl fmt::Display for DemoNetwork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Testnet => "testnet",
            Self::Testnet4 => "testnet4",
            Self::Signet => "signet",
            Self::Regtest => "regtest",
        })
    }
}

struct App {
    /// 256 bits represented as 32 bytes (256 bits total)
    bits: [u8; 32],
    /// Current selected index in the 256-bit array (0..255)
    cursor: usize,
    /// Selected safe Bitcoin network profile
    network: DemoNetwork,
    /// Current art preset
    art: BitArt,
    /// Whether the extra info footer is visible
    show_info: bool,
    /// Whether the help popup is visible
    show_help: bool,
    /// Whether the app should quit
    should_quit: bool,
}

struct DerivedIdentity {
    address: Address,
    wif: String,
    wif_roundtrip_ok: bool,
    address_roundtrip_ok: bool,
    pubkey_match_ok: bool,
}

impl App {
    fn new() -> Self {
        Self {
            bits: [0u8; 32],
            cursor: 0,
            network: DemoNetwork::Testnet,
            art: BitArt::Checker,
            show_info: false,
            show_help: false,
            should_quit: false,
        }
    }

    fn toggle_bit(&mut self) {
        let byte_idx = self.cursor / 8;
        let bit_idx = 7 - (self.cursor % 8);
        self.bits[byte_idx] ^= 1 << bit_idx;
    }

    fn randomize_bits(&mut self) {
        loop {
            OsRng.fill_bytes(&mut self.bits);
            if SecretKey::from_slice(&self.bits).is_ok() {
                break;
            }
        }
    }

    fn clear_bits(&mut self) {
        self.bits = [0u8; 32];
    }

    fn fill_bits(&mut self) {
        self.bits = [0xff; 32];
    }

    fn cycle_art(&mut self) {
        self.art = self.art.next();
        self.apply_art();
    }

    fn apply_art(&mut self) {
        self.bits = [0u8; 32];
        for row in 0..16 {
            for col in 0..16 {
                if self.art_bit(row, col) {
                    let bit_flat_idx = row * 16 + col;
                    let b_idx = bit_flat_idx / 8;
                    let bit_idx = 7 - (bit_flat_idx % 8);
                    self.bits[b_idx] |= 1 << bit_idx;
                }
            }
        }
    }

    fn art_bit(&self, row: usize, col: usize) -> bool {
        let row = row as i32;
        let col = col as i32;
        match self.art {
            BitArt::Checker => (row + col) % 2 == 0,
            BitArt::Border => row == 0 || row == 15 || col == 0 || col == 15,
            BitArt::X => row == col || row + col == 15,
            BitArt::Diamond => {
                let center = 7_i32;
                (row - center).abs() + (col - center).abs() <= 5
            }
            BitArt::Plus => row == 7 || row == 8 || col == 7 || col == 8,
            BitArt::Diagonal => (row + col) % 4 == 0 || (row + col) % 4 == 1,
        }
    }

    fn cycle_network(&mut self) {
        self.network = self.network.next();
    }

    fn toggle_info(&mut self) {
        self.show_info = !self.show_info;
    }

    fn move_cursor(&mut self, dx: i32, dy: i32) {
        let col = (self.cursor % 16) as i32;
        let row = (self.cursor / 16) as i32;

        let new_col = (col + dx).rem_euclid(16);
        let new_row = (row + dy).rem_euclid(16);

        self.cursor = (new_row * 16 + new_col) as usize;
    }

    fn secret_key(&self) -> Result<SecretKey, bitcoin::secp256k1::Error> {
        SecretKey::from_slice(&self.bits)
    }

    fn derived_identity(&self) -> Result<DerivedIdentity, String> {
        let secret_key = self
            .secret_key()
            .map_err(|err| format!("invalid secret key: {err}"))?;

        let secp = Secp256k1::new();
        let private_key = PrivateKey::new(secret_key, self.network.as_bitcoin_network());
        let public_key = private_key.public_key(&secp);
        let compressed_public_key = CompressedPublicKey::try_from(public_key)
            .map_err(|err| format!("could not compress public key: {err}"))?;
        let address = Address::p2wpkh(&compressed_public_key, self.network.as_bitcoin_network());
        let wif = private_key.to_wif();
        let parsed_wif =
            PrivateKey::from_wif(&wif).map_err(|err| format!("WIF roundtrip failed: {err}"))?;
        let parsed_address: Address<NetworkUnchecked> = address
            .to_string()
            .parse()
            .map_err(|err| format!("address roundtrip failed: {err}"))?;

        Ok(DerivedIdentity {
            address: address.clone(),
            wif,
            wif_roundtrip_ok: parsed_wif.inner == private_key.inner
                && parsed_wif.compressed == private_key.compressed
                && parsed_wif.network == private_key.network,
            address_roundtrip_ok: parsed_address
                .require_network(self.network.as_bitcoin_network())
                .is_ok(),
            pubkey_match_ok: address.is_related_to_pubkey(&public_key),
        })
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    while !app.should_quit {
        terminal.draw(|f| ui(f, &mut app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.show_info {
                        match key.code {
                            KeyCode::Char('q') => app.should_quit = true,
                            KeyCode::Esc
                            | KeyCode::Char('\\')
                            | KeyCode::Char('|')
                            | KeyCode::Char('i') => app.toggle_info(),
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => app.should_quit = true,
                            KeyCode::Esc if app.show_help => app.show_help = false,
                            KeyCode::Char(' ') => app.toggle_bit(),
                            KeyCode::Char('r') => app.randomize_bits(),
                            KeyCode::Char('c') => app.clear_bits(),
                            KeyCode::Char('f') => app.fill_bits(),
                            KeyCode::Char('a') => app.cycle_art(),
                            KeyCode::Char('?') => app.show_help = !app.show_help,
                            KeyCode::Tab => app.cycle_network(),
                            KeyCode::Char('\\') | KeyCode::Char('|') | KeyCode::Char('i') => {
                                app.toggle_info()
                            }
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
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let derived = app.derived_identity();
    let hex_str: String = app.bits.iter().map(|b| format!("{:02x}", b)).collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(18),
            Constraint::Length(4),
        ])
        .split(f.size());

    let byte_idx = app.cursor / 8;
    let bit_idx = 7 - (app.cursor % 8);
    let header_text = format!(
        " BIT GRID | Network: {} | Selected Bit: {} (Byte {}, Bit {}) ",
        app.network, app.cursor, byte_idx, bit_idx
    );
    let header = Paragraph::new(header_text)
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL).title(" BIT GRID "));
    f.render_widget(header, chunks[0]);

    let grid_block = Block::default()
        .borders(Borders::ALL)
        .title(" 16x16 Bit Grid (256 Bits) ");
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

    let current_state_lines = match derived {
        Ok(identity) => vec![
            Line::from(vec![
                Span::styled(
                    "SECRET: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(hex_str.clone(), Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled(
                    "WIF: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(identity.wif, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled(
                    "ADDR: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    identity.address.to_string(),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "VERIFY: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(
                        "pubkey {} | WIF {} | addr {}",
                        if identity.pubkey_match_ok { "ok" } else { "fail" },
                        if identity.wif_roundtrip_ok { "ok" } else { "fail" },
                        if identity.address_roundtrip_ok { "ok" } else { "fail" }
                    ),
                    Style::default().fg(Color::Green),
                ),
            ]),
        ],
        Err(err) => vec![
            Line::from(vec![
                Span::styled(
                    "SECRET: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(hex_str.clone(), Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled(
                    "WIF: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("n/a", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(vec![
                Span::styled(
                    "ADDR: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("invalid secret", Style::default().fg(Color::Red)),
            ]),
            Line::from(vec![
                Span::styled(
                    "VERIFY: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(err, Style::default().fg(Color::Red)),
            ]),
        ],
    };

    let state_footer = Paragraph::new(current_state_lines)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" Current State "));
    f.render_widget(state_footer, chunks[2]);

    if app.show_info {
        let area = centered_rect(86, 78, f.size());
        f.render_widget(Clear, area);

        let modal = Paragraph::new(vec![
            Line::from(" ENTROPY "),
            Line::from("  Type: hexadecimal "),
            Line::from("  Event Count: 64 "),
            Line::from("  Avg Bits/Event: 4.00 "),
            Line::from("  Raw Entropy Words: 24 "),
            Line::from("  Total Bits: 256 "),
            Line::from(format!("  Filtered Entropy: {}", hex_str)),
            Line::from(""),
            Line::from(" KEY / WALLET "),
            Line::from("  Mnemonic Length: 24 "),
            Line::from("  PBKDF2 rounds: 2048 "),
            Line::from("  Coin: testnet / testnet4 / signet / regtest "),
            Line::from("  Derivation Paths: BIP32, BIP44, BIP49, BIP84, BIP141 "),
            Line::from(""),
            Line::from(" CURRENT "),
            Line::from(format!("  Network: {}", app.network)),
            Line::from(format!("  WIF prefix: {}", app.network.wif_prefix())),
            Line::from(format!("  Bech32 HRP: {}", app.network.bech32_hrp())),
            Line::from(format!("  Selected bit: {}", app.cursor)),
            Line::from(""),
            Line::from(" CONTROLS "),
            Line::from("  Tab    Cycle network profile "),
            Line::from("  \\ / | / i  Toggle extra info "),
            Line::from("  r      Randomize a valid secret "),
            Line::from("  f      Fill the entire grid "),
            Line::from("  c      Clear the grid "),
            Line::from("  a      Cycle bit matrix art presets "),
            Line::from("  Space  Flip the selected bit "),
            Line::from("  hjkl   Move the cursor "),
            Line::from("  Esc    Close help "),
            Line::from("  q      Quit "),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Current State "),
        )
        .alignment(Alignment::Left)
        .style(Style::default().fg(Color::White));
        f.render_widget(modal, area);
    }

    if app.show_help {
        let area = centered_rect(82, 76, f.size());
        f.render_widget(Clear, area);

        let help = Paragraph::new(vec![
            Line::from(" BIP39-STYLE INFO "),
            Line::from(""),
            Line::from(" This demo does not generate a mnemonic; it treats the grid as raw 256-bit entropy. "),
            Line::from(" Randomize uses OS-backed CSPRNG and only accepts a valid secp256k1 secret key. "),
            Line::from(""),
            Line::from(" KEY TYPES "),
            Line::from("  entropy  ->  256-bit grid "),
            Line::from("  secret   ->  secp256k1 private key "),
            Line::from("  public   ->  compressed public key "),
            Line::from("  address  ->  native segwit P2WPKH "),
            Line::from(""),
            Line::from(" DERIVATION NOTES "),
            Line::from("  BIP32 roots can feed BIP44/BIP49/BIP84-style paths. "),
            Line::from("  BIP85-style deterministic entropy is informational only here. "),
            Line::from("  Networks are limited to testnet, testnet4, signet, and regtest. "),
            Line::from(""),
            Line::from(" SHORTCUTS "),
            Line::from("  ?      Toggle this help panel "),
            Line::from("  Tab    Cycle network profile "),
            Line::from("  \\ / | / i  Toggle extra info "),
            Line::from("  r      Randomize a valid secret "),
            Line::from("  f      Fill the entire grid "),
            Line::from("  c      Clear the grid "),
            Line::from("  Space  Flip the selected bit "),
            Line::from("  hjkl   Move the cursor "),
            Line::from("  Esc    Close help "),
            Line::from("  q      Quit "),
        ])
        .block(Block::default().borders(Borders::ALL).title(" Help "))
        .alignment(Alignment::Left)
        .style(Style::default().fg(Color::White));
        f.render_widget(help, area);
    }
}

fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    area: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_safe_networks() {
        let mut network = DemoNetwork::Testnet;
        network = network.next();
        assert_eq!(network, DemoNetwork::Testnet4);
        network = network.next();
        assert_eq!(network, DemoNetwork::Signet);
        network = network.next();
        assert_eq!(network, DemoNetwork::Regtest);
        network = network.next();
        assert_eq!(network, DemoNetwork::Testnet);
    }

    #[test]
    fn derives_and_roundtrips_identity() {
        let mut app = App::new();
        app.bits = [1u8; 32];
        app.network = DemoNetwork::Regtest;

        let identity = app.derived_identity().expect("valid identity");
        assert!(identity.pubkey_match_ok);
        assert!(identity.wif_roundtrip_ok);
        assert!(identity.address_roundtrip_ok);
    }

    #[test]
    fn fill_bits_fills_entire_grid() {
        let mut app = App::new();
        app.fill_bits();
        assert!(app.bits.iter().all(|byte| *byte == 0xff));
        assert!(app.secret_key().is_err());
    }
}
