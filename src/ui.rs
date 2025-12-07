use crate::fetch::FetchState;
use crate::models::Pokemon;
use crate::utils::{format_name, text_to_lines};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Span, Spans};
use ratatui::widgets::Gauge;
use ratatui::widgets::{
    Block, Borders, List, ListItem, Paragraph, Wrap,
};
use std::collections::HashMap;
use image::imageops::FilterType;
use ratatui::Terminal;
use std::io;
use std::io::Stdout;
use std::sync::{Arc, Mutex};

/// Compact RGB thumbnail stored in the in-memory cache.
pub struct SpriteThumb {
    pub w: u32,
    pub h: u32,
    /// RGB pixels in row-major order (len = w*h*3)
    pub pixels: Vec<u8>,
}

pub struct App {
    pub all_pokemons: Vec<Pokemon>,
    pub visible: Vec<usize>, // indices into all_pokemons
    pub selected_visible: usize,
    pub search_mode: bool,
    pub search_query: String,
    pub fetch_state: Option<Arc<Mutex<FetchState>>>,
    pub show_sprites: bool,
    pub show_help: bool,
    // in-memory cache of compact resized sprite thumbnails (RGB bytes).
    // Use an Arc<Mutex<...>> so a background thread can populate the cache
    // without blocking the UI thread.
    pub sprite_cache: std::sync::Arc<std::sync::Mutex<HashMap<u32, SpriteThumb>>>,
}

impl App {
    pub fn new(all: Vec<Pokemon>) -> Self {
        let visible = (0..all.len()).collect();
        Self {
            all_pokemons: all,
            visible,
            selected_visible: 0,
            search_mode: false,
            search_query: String::new(),
            fetch_state: None,
            show_sprites: true,
            show_help: false,
            sprite_cache: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Load (or generate) a compact thumbnail for `id` and return pixel rows sized `w` x `h`.
    ///
    /// The cache stores a small thumbnail (e.g., 48x48 RGB). If the requested size
    /// matches the cached thumb, we return its pixels directly. If the requested size
    /// differs, we perform an in-memory resize from the cached thumbnail which is
    /// much cheaper than reloading full images from disk.
    pub fn get_sprite_pixels(&self, id: u32, w: u32, h: u32) -> Option<Vec<Vec<(u8, u8, u8)>>> {
        const THUMB_W: u32 = 48;
        const THUMB_H: u32 = 48;

        // Attempt to get the thumbnail from cache; if missing, load and insert a thumb.
        let mut cache = self.sprite_cache.lock().unwrap();
        if !cache.contains_key(&id) {
            let path = format!("data/sprites/{}.png", id);
            if let Ok(img) = image::open(&path) {
                // Resize to canonical thumbnail size and store RGB bytes.
                let small = image::imageops::resize(&img.to_rgba8(), THUMB_W, THUMB_H, FilterType::Lanczos3);
                let mut pixels = Vec::with_capacity((THUMB_W * THUMB_H * 3) as usize);
                for y in 0..small.height() {
                    for x in 0..small.width() {
                        let p = small.get_pixel(x, y);
                        pixels.push(p[0]);
                        pixels.push(p[1]);
                        pixels.push(p[2]);
                    }
                }
                cache.insert(
                    id,
                    SpriteThumb {
                        w: THUMB_W,
                        h: THUMB_H,
                        pixels,
                    },
                );
            } else {
                return None;
            }
        }

        // We have a thumbnail; if requested size equals thumb size, return directly.
        if let Some(thumb) = cache.get(&id) {
            // If exact match, iterate rows directly.
            if thumb.w == w && thumb.h == h {
                let mut rows: Vec<Vec<(u8, u8, u8)>> = Vec::with_capacity(h as usize);
                for y in 0..h {
                    let mut row = Vec::with_capacity(w as usize);
                    let row_start = (y * w * 3) as usize;
                    for x in 0..w {
                        let idx = row_start + (x as usize) * 3;
                        row.push((thumb.pixels[idx], thumb.pixels[idx + 1], thumb.pixels[idx + 2]));
                    }
                    rows.push(row);
                }
                return Some(rows);
            }

            // Otherwise, perform an in-memory resize from the thumbnail to the requested size.
            // Build an ImageBuffer from the RGB bytes (add opaque alpha) and use image::resize.
            let mut buf = image::RgbaImage::new(thumb.w, thumb.h);
            for y in 0..thumb.h {
                for x in 0..thumb.w {
                    let idx = ((y * thumb.w + x) * 3) as usize;
                    let r = thumb.pixels[idx];
                    let g = thumb.pixels[idx + 1];
                    let b = thumb.pixels[idx + 2];
                    buf.put_pixel(x, y, image::Rgba([r, g, b, 255]));
                }
            }
            let resized = image::imageops::resize(&buf, w, h, FilterType::Lanczos3);
            let mut rows: Vec<Vec<(u8, u8, u8)>> = Vec::with_capacity(resized.height() as usize);
            for y in 0..resized.height() {
                let mut row = Vec::with_capacity(resized.width() as usize);
                for x in 0..resized.width() {
                    let p = resized.get_pixel(x, y);
                    row.push((p[0], p[1], p[2]));
                }
                rows.push(row);
            }
            return Some(rows);
        }

        None
    }

    pub fn next(&mut self) {
        if !self.visible.is_empty() {
            self.selected_visible = (self.selected_visible + 1) % self.visible.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.visible.is_empty() {
            if self.selected_visible == 0 {
                self.selected_visible = self.visible.len() - 1;
            } else {
                self.selected_visible -= 1;
            }
        }
    }



    pub fn apply_filter(&mut self) {
        let q = self.search_query.to_lowercase();
        if q.is_empty() {
            self.visible = (0..self.all_pokemons.len()).collect();
        } else {
            self.visible = self
                .all_pokemons
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    if p.name.to_lowercase().contains(&q)
                        || p.types.iter().any(|t| t.to_lowercase().contains(&q))
                    {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect();
        }

        if self.visible.is_empty() {
            self.selected_visible = 0;
        } else if self.selected_visible >= self.visible.len() {
            self.selected_visible = self.visible.len() - 1;
        }
    }
}

pub fn draw_ui(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> io::Result<()> {
    terminal
        .draw(|f| {
            // helper to compute a centered rect for popups
            fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
                let popup_w = r.width.saturating_mul(percent_x) / 100;
                let popup_h = r.height.saturating_mul(percent_y) / 100;
                let popup_x = r.x + (r.width.saturating_sub(popup_w) / 2);
                let popup_y = r.y + (r.height.saturating_sub(popup_h) / 2);
                Rect::new(popup_x, popup_y, popup_w, popup_h)
            }
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                .split(size);

            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)])
                .split(chunks[0]);

            let items: Vec<ListItem> = app
                .visible
                .iter()
                .filter_map(|&i| app.all_pokemons.get(i))
                .map(|p| {
                    let display_name = format_name(&p.name);
                    let lines = vec![Spans::from(vec![Span::raw(format!(
                        "#{} {}",
                        p.pokedex, display_name
                    ))])];
                    ListItem::new(lines)
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Pokémon"))
                .highlight_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );

            f.render_stateful_widget(list, left_chunks[0], &mut {
                let mut state = ratatui::widgets::ListState::default();
                if !app.visible.is_empty() {
                    state.select(Some(app.selected_visible));
                }
                state
            });

            if let Some(state) = &app.fetch_state {
                let st = state.lock().unwrap();
                if st.in_progress {
                    let pct = if st.total == 0 {
                        0.0
                    } else {
                        st.fetched as f64 / st.total as f64
                    };
                    let gauge = Gauge::default()
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title("Fetching Pokémon"),
                        )
                        .gauge_style(Style::default().fg(Color::Green))
                        .ratio(pct);
                    f.render_widget(gauge, left_chunks[1]);
                } else {
                    let search_para = if app.search_mode {
                        Paragraph::new(vec![Spans::from(Span::raw(format!(
                            "/{}",
                            app.search_query
                        )))])
                        .block(Block::default().borders(Borders::ALL).title("Search"))
                    } else {
                        Paragraph::new(vec![Spans::from(Span::raw(
                            "Press '/' to search. Type to filter by name or type.",
                        ))])
                        .block(Block::default().borders(Borders::ALL).title("Search"))
                    };
                    f.render_widget(search_para, left_chunks[1]);
                }
            } else {
                let search_para = if app.search_mode {
                    Paragraph::new(vec![Spans::from(Span::raw(format!(
                        "/{}",
                        app.search_query
                    )))])
                    .block(Block::default().borders(Borders::ALL).title("Search"))
                } else {
                    Paragraph::new(vec![Spans::from(Span::raw(
                        "Press '/' to search. Type to filter by name or type.",
                    ))])
                    .block(Block::default().borders(Borders::ALL).title("Search"))
                };
                f.render_widget(search_para, left_chunks[1]);
            }

            let detail = if !app.visible.is_empty() {
                let sel_idx = app.visible[app.selected_visible];
                let detail_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(12), Constraint::Min(6)])
                    .split(chunks[1]);

                let top_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Length(24), Constraint::Min(10)])
                    .split(detail_chunks[0]);

                // Sprite widget
                let sprite_para = if app.show_sprites {
                    let rect = top_chunks[0];
                    let avail_w = if rect.width > 2 {
                        (rect.width - 2) as u32
                    } else {
                        1
                    };
                    let avail_h = if rect.height > 2 {
                        (rect.height - 2) as u32
                    } else {
                        1
                    };
                    let sprite_w = std::cmp::min(avail_w, 64u32);
                    let sprite_h = std::cmp::min(avail_h, 64u32);

                    // get pokedex id first (copy) then call mutable loader
                    let pokedex_id = app.all_pokemons[sel_idx].pokedex;
                    if let Some(sprite_lines) = app.get_sprite_pixels(pokedex_id, sprite_w, sprite_h) {
                        let mut stext: Vec<Spans> = Vec::new();
                        for row in sprite_lines.iter() {
                            let mut spans = Vec::new();
                            for &(r, g, b) in row.iter() {
                                spans.push(Span::styled(
                                    " ",
                                    Style::default().bg(Color::Rgb(r, g, b)),
                                ));
                            }
                            stext.push(Spans::from(spans));
                        }
                        Paragraph::new(stext)
                            .block(Block::default().borders(Borders::ALL).title("Sprite"))
                    } else {
                        Paragraph::new("(no sprite)")
                            .block(Block::default().borders(Borders::ALL).title("Sprite"))
                    }
                } else {
                    Paragraph::new("(sprites off)")
                        .block(Block::default().borders(Borders::ALL).title("Sprite"))
                };
                f.render_widget(sprite_para, top_chunks[0]);

                let p = &app.all_pokemons[sel_idx];
                let mut info_lines: Vec<Spans> = Vec::new();
                info_lines.push(Spans::from(Span::styled(
                    format!("{} (#{})", format_name(&p.name), p.pokedex),
                    Style::default().add_modifier(Modifier::BOLD),
                )));
                // Render types as colored badges
                let mut type_spans: Vec<Span> = Vec::new();
                type_spans.push(Span::raw("Types: "));
                for (i, t) in p.types.iter().enumerate() {
                    let t_l = t.to_lowercase();
                    // color mapping using nicer RGB values per type
                    let (r, g, b) = match t_l.as_str() {
                        "normal" => (168, 168, 120),
                        "fire" => (240, 128, 48),
                        "water" => (104, 144, 240),
                        "grass" => (120, 200, 80),
                        "electric" => (248, 208, 48),
                        "ice" => (152, 216, 216),
                        "fighting" => (192, 48, 40),
                        "poison" => (160, 64, 160),
                        "ground" => (224, 192, 104),
                        "flying" => (168, 144, 240),
                        "psychic" => (248, 88, 136),
                        "bug" => (168, 184, 32),
                        "rock" => (184, 160, 56),
                        "ghost" => (112, 88, 152),
                        "dragon" => (112, 56, 248),
                        "dark" => (112, 88, 72),
                        "steel" => (184, 184, 208),
                        "fairy" => (238, 153, 172),
                        _ => (200, 200, 200),
                    };
                    let bg = Color::Rgb(r, g, b);
                    // choose contrasting foreground (black or white)
                    let lum = 0.2126 * (r as f32) + 0.7152 * (g as f32) + 0.0722 * (b as f32);
                    let fg = if lum > 160.0 { Color::Black } else { Color::White };
                    // small padded badge
                    type_spans.push(Span::styled(
                        format!(" {} ", format_name(t)),
                        Style::default().fg(fg).bg(bg),
                    ));
                    if i < p.types.len() - 1 {
                        type_spans.push(Span::raw(" "));
                    }
                }
                info_lines.push(Spans::from(type_spans));
                if !p.abilities.is_empty() {
                    info_lines.push(Spans::from(Span::raw(format!(
                        "Abilities: {}",
                        p.abilities.join(", ")
                    ))));
                }
                info_lines.push(Spans::from(Span::raw(format!(
                    "Height: {}  Weight: {}  Base EXP: {}",
                    p.height, p.weight, p.base_experience
                ))));
                let info_para = Paragraph::new(info_lines)
                    .block(Block::default().borders(Borders::ALL).title("Info"))
                    .wrap(Wrap { trim: true });
                f.render_widget(info_para, top_chunks[1]);

                let bottom_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Length(30), Constraint::Min(10)])
                    .split(detail_chunks[1]);

                // Render per-stat horizontal bars aligned with each stat name.
                // We'll draw lines with: NAME (padded) | VALUE | [bar...]
                let stats_rect = bottom_chunks[0];
                let inner_w = if stats_rect.width > 2 {
                    (stats_rect.width - 2) as usize
                } else {
                    1usize
                };

                // Reserve columns: name (10), value (4), spaces (2)
                let name_w = 10usize;
                let val_w = 4usize;
                let reserved = name_w + val_w + 2;
                let bar_max_w = inner_w.saturating_sub(reserved);

                // find a global max across all Pokémon so bars are comparable across entries
                let global_max = app
                    .all_pokemons
                    .iter()
                    .flat_map(|pp| pp.stats.iter().map(|s| s.base))
                    .max()
                    .unwrap_or(1) as f32;
                // cap scale to a reasonable upper bound (e.g., 255) and avoid zero
                let scale_max = global_max.clamp(1.0, 255.0);

                let mut stat_lines: Vec<Spans> = Vec::new();
                for st in p.stats.iter() {
                    // short name/abbrev
                    let nm = match st.name.as_str() {
                        "hp" => "HP".to_string(),
                        "attack" => "ATK".to_string(),
                        "defense" => "DEF".to_string(),
                        "special-attack" => "SpA".to_string(),
                        "special-defense" => "SpD".to_string(),
                        "speed" => "SPD".to_string(),
                        other => {
                            // fallback: capitalize first letter
                            let mut c = other.chars();
                            match c.next() {
                                None => String::new(),
                                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                            }
                        }
                    };

                    let bar_len = if scale_max <= 0.0 {
                        0
                    } else {
                        (((st.base as f32) / scale_max) * (bar_max_w as f32)).round() as usize
                    };
                    let bar = "█".repeat(bar_len);

                    let line = format!(
                        "{:<name_w$} {:>val_w$} {}",
                        nm,
                        st.base,
                        bar,
                        name_w = name_w,
                        val_w = val_w
                    );
                    stat_lines.push(Spans::from(Span::raw(line)));
                }

                let stats_para = Paragraph::new(stat_lines)
                    .block(Block::default().borders(Borders::ALL).title("Stats"));
                f.render_widget(stats_para, stats_rect);

                let mut right_text: Vec<Spans> = Vec::new();
                right_text.push(Spans::from(Span::styled(
                    "Description:",
                    Style::default().add_modifier(Modifier::BOLD),
                )));
                for line in text_to_lines(&p.description, 60) {
                    right_text.push(Spans::from(Span::raw(line)));
                }
                right_text.push(Spans::from(Span::raw("")));
                // AI Summary removed — leave space for potential replacement.
                let right_para = Paragraph::new(right_text)
                    .block(Block::default().borders(Borders::ALL).title("Details"))
                    .wrap(Wrap { trim: true });
                f.render_widget(right_para, bottom_chunks[1]);

                Paragraph::new("").block(Block::default())
            } else {
                Paragraph::new("No Pokémon match the filter")
                    .block(Block::default().borders(Borders::ALL).title("Details"))
            };

            f.render_widget(detail, chunks[1]);

            // If help is requested, draw a centered help modal on top
            if app.show_help {
                let area = f.size();
                let popup = centered_rect(60, 40, area);
                let mut help_lines: Vec<Spans> = Vec::new();
                help_lines.push(Spans::from(Span::styled(
                    "Keybindings",
                    Style::default().add_modifier(Modifier::BOLD),
                )));
                help_lines.push(Spans::from(Span::raw("")));
                help_lines.push(Spans::from(Span::raw("q       Quit")));
                help_lines.push(Spans::from(Span::raw("/       Enter search mode")));
                help_lines.push(Spans::from(Span::raw(
                    "Enter/Esc  Finish or cancel search mode",
                )));
                help_lines.push(Spans::from(Span::raw("Up/Down Navigate list")));
                help_lines.push(Spans::from(Span::raw("r       Refresh fetch (background)")));
                help_lines.push(Spans::from(Span::raw("?       Toggle this help modal")));
                help_lines.push(Spans::from(Span::raw("")));
                help_lines.push(Spans::from(Span::raw(
                    "Use arrow keys to navigate and Enter to focus details.",
                )));

                let help_para = Paragraph::new(help_lines)
                    .block(Block::default().borders(Borders::ALL).title("Help"))
                    .wrap(Wrap { trim: true });
                f.render_widget(help_para, popup);
            }
        })
        .map(|_| ())
}
