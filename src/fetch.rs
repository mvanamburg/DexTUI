use crate::models::Pokemon;
use reqwest;
use serde_json;
use std::error::Error;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct FetchState {
    pub in_progress: bool,
    pub fetched: usize,
    pub total: usize,
}

pub async fn fetch_and_cache(
    limit: usize,
    state: Option<Arc<Mutex<FetchState>>>,
) -> Result<Vec<Pokemon>, Box<dyn Error>> {
    let cache_path = "data/pokemon.json";
    if let Ok(existing) = std::fs::read_to_string(cache_path) {
        if let Ok(mut v) = serde_json::from_str::<Vec<Pokemon>>(&existing) {
            if v.len() >= limit {
                if let Some(s) = &state {
                    let mut st = s.lock().unwrap();
                    st.in_progress = true;
                    st.total = v.len();
                    st.fetched = 0;
                }
                let client = reqwest::Client::new();
                for (i, p) in v.iter_mut().enumerate() {
                    let sprite_path = format!("data/sprites/{}.png", p.pokedex);
                    if p.sprite.is_none()
                        || !std::path::Path::new(&sprite_path).exists()
                        || p.abilities.is_empty()
                        || p.stats.is_empty()
                        || p.height == 0
                        || p.weight == 0
                        || p.base_experience == 0
                    {
                        let poke_url = format!("https://pokeapi.co/api/v2/pokemon/{}", p.name);
                        if let Ok(p_res) = client.get(&poke_url).send().await {
                            if let Ok(p_json) = p_res.json::<serde_json::Value>().await {
                                if let Some(sprite_url) = p_json
                                    .get("sprites")
                                    .and_then(|s| s.get("front_default"))
                                    .and_then(|u| u.as_str())
                                {
                                    if let Ok(resp) = client.get(sprite_url).send().await {
                                        if let Ok(bytes) = resp.bytes().await {
                                            let _ = std::fs::create_dir_all("data/sprites");
                                            let _ = std::fs::write(&sprite_path, &bytes);
                                            p.sprite = Some(sprite_url.to_string());
                                        }
                                    }
                                }

                                let abilities = p_json
                                    .get("abilities")
                                    .and_then(|a| a.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|it| {
                                                it.get("ability")
                                                    .and_then(|ab| ab.get("name"))
                                                    .and_then(|n| n.as_str())
                                                    .map(|s| s.to_string())
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_else(|| vec![]);
                                let height =
                                    p_json.get("height").and_then(|v| v.as_u64()).unwrap_or(0)
                                        as u32;
                                let weight =
                                    p_json.get("weight").and_then(|v| v.as_u64()).unwrap_or(0)
                                        as u32;
                                let base_experience = p_json
                                    .get("base_experience")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32;
                                let stats = p_json
                                    .get("stats")
                                    .and_then(|s| s.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|it| {
                                                let name = it
                                                    .get("stat")
                                                    .and_then(|st| st.get("name"))
                                                    .and_then(|n| n.as_str())?;
                                                let base =
                                                    it.get("base_stat").and_then(|b| b.as_u64())?
                                                        as u32;
                                                Some(crate::models::Stat {
                                                    name: name.to_string(),
                                                    base,
                                                })
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_else(|| vec![]);

                                p.abilities = abilities;
                                p.height = height;
                                p.weight = weight;
                                p.base_experience = base_experience;
                                p.stats = stats;
                            }
                        }
                    }
                    if let Some(s) = &state {
                        let mut st = s.lock().unwrap();
                        st.fetched = i + 1;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                if let Ok(sv) = serde_json::to_string_pretty(&v) {
                    let _ = std::fs::write(cache_path, sv);
                }
                if let Some(s) = &state {
                    let mut st = s.lock().unwrap();
                    st.in_progress = false;
                }
                return Ok(v);
            }
        }
    }

    if let Some(s) = &state {
        let mut st = s.lock().unwrap();
        st.in_progress = true;
        st.fetched = 0;
        st.total = limit;
    }

    eprintln!("Fetching Pokémon from PokeAPI (this may take a while)...");
    let client = reqwest::Client::new();
    let list_url = format!("https://pokeapi.co/api/v2/pokemon?limit={}", limit);
    let list_res = client.get(&list_url).send().await?;
    let list_json: serde_json::Value = list_res.json().await?;
    let results = list_json
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or("unexpected list response")?;

    let mut pokemons: Vec<Pokemon> = Vec::new();
    for entry in results.iter() {
        if let Some(name) = entry.get("name").and_then(|n| n.as_str()) {
            let poke_url = format!("https://pokeapi.co/api/v2/pokemon/{}", name);
            let p_res = client.get(&poke_url).send().await;
            if p_res.is_err() {
                eprintln!("failed to fetch {}: {}", name, p_res.unwrap_err());
                continue;
            }
            let p_json: serde_json::Value = p_res.unwrap().json().await?;
            let id = p_json.get("id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let sprite_url = p_json
                .get("sprites")
                .and_then(|s| s.get("front_default"))
                .and_then(|u| u.as_str())
                .map(|s| s.to_string());
            let types = p_json
                .get("types")
                .and_then(|t| t.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|it| {
                            it.get("type")
                                .and_then(|ty| ty.get("name"))
                                .and_then(|n| n.as_str())
                                .map(|s| s.to_string())
                        })
                        .collect()
                })
                .unwrap_or_else(|| vec![]);

            let abilities = p_json
                .get("abilities")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|it| {
                            it.get("ability")
                                .and_then(|ab| ab.get("name"))
                                .and_then(|n| n.as_str())
                                .map(|s| s.to_string())
                        })
                        .collect()
                })
                .unwrap_or_else(|| vec![]);
            let height = p_json.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let weight = p_json.get("weight").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let base_experience = p_json
                .get("base_experience")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let stats = p_json
                .get("stats")
                .and_then(|s| s.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|it| {
                            let name = it
                                .get("stat")
                                .and_then(|st| st.get("name"))
                                .and_then(|n| n.as_str())?;
                            let base = it.get("base_stat").and_then(|b| b.as_u64())? as u32;
                            Some(crate::models::Stat {
                                name: name.to_string(),
                                base,
                            })
                        })
                        .collect()
                })
                .unwrap_or_else(|| vec![]);

            let species_url = format!("https://pokeapi.co/api/v2/pokemon-species/{}", name);
            let s_res = client.get(&species_url).send().await;
            let description = if let Ok(sresp) = s_res {
                let s_json: serde_json::Value =
                    sresp.json().await.unwrap_or(serde_json::Value::Null);
                if let Some(entries) = s_json.get("flavor_text_entries").and_then(|e| e.as_array())
                {
                    let mut found = None;
                    for ent in entries {
                        if ent
                            .get("language")
                            .and_then(|l| l.get("name"))
                            .and_then(|n| n.as_str())
                            == Some("en")
                        {
                            if let Some(ft) = ent.get("flavor_text").and_then(|f| f.as_str()) {
                                found = Some(ft.replace('\n', " ").replace('\u{c}', " "));
                                break;
                            }
                        }
                    }
                    found.unwrap_or_else(|| "No description available.".to_string())
                } else {
                    "No description available.".to_string()
                }
            } else {
                "No description available.".to_string()
            };

            pokemons.push(Pokemon {
                name: name.to_string(),
                pokedex: id,
                types,
                description,
                sprite: sprite_url.clone(),
                abilities,
                height,
                weight,
                base_experience,
                stats,
            });

            if let Some(url) = sprite_url {
                if let Ok(resp) = client.get(&url).send().await {
                    if let Ok(bytes) = resp.bytes().await {
                        let _ = std::fs::create_dir_all("data/sprites");
                        let _ = std::fs::write(format!("data/sprites/{}.png", id), &bytes);
                    }
                }
            }
            if let Some(s) = &state {
                let mut st = s.lock().unwrap();
                st.fetched = pokemons.len();
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    if let Ok(s) = serde_json::to_string_pretty(&pokemons) {
        let _ = std::fs::create_dir_all("data");
        let _ = std::fs::write(cache_path, s);
    }
    if let Some(s) = &state {
        let mut st = s.lock().unwrap();
        st.in_progress = false;
        st.fetched = pokemons.len();
        st.total = pokemons.len();
    }

    Ok(pokemons)
}
