# Copilot Instructions

## Build, test, and lint

- `cargo run` — start the TUI.
- `cargo test` — run the full test suite.
- `cargo test it_works` — run the single unit test in `src/main.rs`.
- `cargo fmt --check` — check formatting.
- `cargo clippy -- -D warnings` — lint the crate.

## High-level architecture

- This is a single-binary Rust TUI built with `crossterm` + `ratatui`.
- `src/main.rs` contains the whole app: terminal setup/teardown, event loop, app state, rendering, and tests.
- `App` stores 256 bits as `[u8; 32]`, a cursor index, and quit state.
- The UI renders a 16x16 bit grid, a header with the selected bit, and a footer with the hex state, WIF/address verification, and extra network metadata.
- Input is handled in the main loop with polling; `q` quits, `Esc` closes help when open, arrow keys or `hjkl` move, space toggles, `r` randomizes a valid test-network secret, `f` fills the entire grid, `c` clears, `Tab` cycles testnet/testnet4/signet/regtest, and `\`/`|`/`i` toggles extra info; `?` opens the help popup.
- The help popup is intentionally BIP39-style: it explains entropy, key-type flow, and derivation-path context without generating mnemonics or touching mainnet.

## Key conventions

- Bit indexing is big-endian within each byte: `bit_idx = 7 - (cursor % 8)`.
- The cursor maps linearly to the grid as `row * 16 + col`; keep that mapping consistent when changing rendering or movement.
- Randomization should use OS-backed randomness (`OsRng`) rather than a deterministic PRNG.
- Key derivation is intentionally restricted to safe Bitcoin networks only; the UI should continue to avoid mainnet generation.
- The footer should keep showing verification status plus a compact derivation summary; the popup can carry the longer explanatory text.
- Terminal setup uses raw mode and the alternate screen; any future control-flow changes should preserve clean teardown.
- The crate is intentionally minimal: avoid adding extra modules or abstractions unless the app grows beyond a single-file TUI.
