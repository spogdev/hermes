# Hermes Client launcher

Rust launcher (Windows 10/11) for Fabric Minecraft 26.1.2. UI is egui/eframe (glow renderer).

## UI rules (from the project owner)

1. **One consistent design across the whole app** unless the owner says otherwise for a specific screen.
2. **All user-facing text goes through language keys**, Minecraft Java style (`key -> text` per language file).
   Do not write any translations or English text yourself. Every text element shows its key
   (e.g. `menu.start`) until the owner says what it should say after reviewing the build.
3. **Placeholders are fine.** If assets or instructions are missing for a feature or element,
   build a clearly marked placeholder or leave it incomplete instead of inventing a design.
4. **Design reference lives in [app-design.md](app-design.md).** Record every screenshot, colour,
   layout or other design detail the owner provides there, and reread it before and during UI work.

## Testing

- `cargo test`: unit tests.
- `cargo run -- --headless`: runs the Start-button launch path without the window, prints
  timings, waits for the game, fails if it crashes. Game output: `%APPDATA%\.hermes\logs\game-output.log`.
- Visual check (debug builds): `cargo run -- --screenshot shot.png [--open-dropdown]` renders the
  window, saves it as a PNG once animations settle, and exits.

## Code map

- `theme.rs`: the shared design (colours, radii, template-to-window layout). Change looks here.
- `lang.rs` + `assets/lang/*.json`: language keys. Add every new key to `en_us.json` with an
  empty value and list it in `app-design.md`.
- `dropdown.rs`: the one dropdown component (glass, expand in place, arrow-to-check morph).
- `ui.rs`: main window. `launch.rs`: install + start the game, reports progress as `Step`.
