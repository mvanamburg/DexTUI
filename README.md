# pokemon_ai_tui

A small Rust TUI using `ratatui` that lists Pokémon on the left and shows details + a mock AI-generated summary on the right.

## Build

```bash
cd /home/matthew/pokemon_ai_tui
cargo build --release
```

## Run

```bash
cargo run --release
```

Controls
- Up/Down: navigate list
- q: quit

Notes
- The AI summary is a local deterministic function `mock_ai_summary`. To integrate a real AI service (OpenAI, etc.), add an async HTTP client (e.g. `reqwest` + `tokio`) and replace the mock function with a call to the API.
- If you open the project in VS Code, open the folder `/home/matthew/pokemon_ai_tui` to work on it interactively.
