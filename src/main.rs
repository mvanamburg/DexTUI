mod fetch;
mod models;
mod ui;
mod utils;

use crossterm::event::{self, Event as CEvent, KeyCode};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::error::Error;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::fetch::{fetch_and_cache, FetchState};
use crate::ui::{draw_ui, App, SpriteThumb};
use crate::utils::load_data;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Determine how many Pokémon to fetch (default 151). Can be configured
    // via the `POKEMON_LIMIT` environment variable.
    let fetch_limit: usize = std::env::var("POKEMON_LIMIT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(151);

    // Support a CLI argument to only fetch and exit (useful for seeding data).
    let args: Vec<String> = std::env::args().collect();
    let fetch_only = args.iter().any(|a| a == "--fetch-only");
    if fetch_only {
        eprintln!("Running fetch-only mode (limit={})...", fetch_limit);
        let pokes = fetch_and_cache(fetch_limit, None).await?;
        eprintln!(
            "Fetch complete: {} pokémon saved to data/pokemon.json",
            pokes.len()
        );
        return Ok(());
    }

    // Start a background fetch (it will skip if cache already has enough)
    let fetch_state = Arc::new(Mutex::new(FetchState {
        in_progress: false,
        fetched: 0,
        total: 0,
    }));
    let fetch_state_clone = fetch_state.clone();
    // Shared slot where background fetch writes updated data for the main loop to pick up
    let updated_data: Arc<Mutex<Option<Vec<models::Pokemon>>>> = Arc::new(Mutex::new(None));
    let updated_data_clone = updated_data.clone();
    let fl_clone = fetch_limit;
    tokio::spawn(async move {
        if let Ok(p) = fetch_and_cache(fl_clone, Some(fetch_state_clone)).await {
            let mut slot = updated_data_clone.lock().unwrap();
            *slot = Some(p);
        }
    });

    // Load data (may be partial until fetch completes)
    let pokemons = load_data("data/pokemon.json").unwrap_or_else(|e| {
        eprintln!("Failed to load data: {}", e);
        vec![]
    });

    // Terminal init
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(pokemons);
    app.fetch_state = Some(fetch_state.clone());

    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    loop {
        draw_ui(&mut terminal, &mut app)?;

        // If a background fetch has produced updated data, pick it up and refresh the app
        if let Some(new) = {
            let mut slot = updated_data.lock().unwrap();
            slot.take()
        } {
            app.all_pokemons = new;
            app.apply_filter();

            // Preload compact thumbnails into the in-memory cache on a background
            // thread so the first time the user views a Pokémon the UI doesn't
            // block waiting for disk I/O. We generate small RGB thumbnails to
            // keep memory usage low.
            let cache_arc = app.sprite_cache.clone();
            let ids: Vec<u32> = app.all_pokemons.iter().map(|p| p.pokedex).collect();
            std::thread::spawn(move || {
                const THUMB_W: u32 = 48;
                const THUMB_H: u32 = 48;
                for id in ids {
                    let path = format!("data/sprites/{}.png", id);
                    if let Ok(img) = image::open(&path) {
                        let small = image::imageops::resize(&img.to_rgba8(), THUMB_W, THUMB_H, image::imageops::FilterType::Lanczos3);
                        let mut pixels = Vec::with_capacity((THUMB_W * THUMB_H * 3) as usize);
                        for y in 0..small.height() {
                            for x in 0..small.width() {
                                let p = small.get_pixel(x, y);
                                pixels.push(p[0]);
                                pixels.push(p[1]);
                                pixels.push(p[2]);
                            }
                        }
                        let thumb = SpriteThumb { w: THUMB_W, h: THUMB_H, pixels };
                        let mut cache = cache_arc.lock().unwrap();
                        cache.insert(id, thumb);
                    }
                }
            });
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                if app.search_mode {
                    match key.code {
                        KeyCode::Enter | KeyCode::Esc => {
                            app.search_mode = false;
                        }
                        KeyCode::Backspace => {
                            app.search_query.pop();
                            app.apply_filter();
                        }
                        KeyCode::Char(c) => {
                            app.search_query.push(c);
                            app.apply_filter();
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::F(1) | KeyCode::Char('h') => {
                            app.show_help = !app.show_help;
                        }
                        KeyCode::Char('/') => {
                            app.search_mode = true;
                            app.search_query.clear();
                            app.apply_filter();
                        }
                        KeyCode::Down => app.next(),
                        KeyCode::Char('r') => {
                            // Trigger a refresh: spawn a background fetch that writes into updated_data
                            let fetch_state_clone2 = fetch_state.clone();
                            let updated_data_clone2 = updated_data.clone();
                            let fl_clone2 = fetch_limit;
                            tokio::spawn(async move {
                                if let Ok(p) =
                                    fetch_and_cache(fl_clone2, Some(fetch_state_clone2)).await
                                {
                                    let mut slot = updated_data_clone2.lock().unwrap();
                                    *slot = Some(p);
                                }
                            });
                        }
                        KeyCode::Up => app.previous(),
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(LeaveAlternateScreen)?;
    Ok(())
}
