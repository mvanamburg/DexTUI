use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Pokemon {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub pokedex: u32,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub sprite: Option<String>,
    #[serde(default)]
    pub abilities: Vec<String>,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub weight: u32,
    #[serde(default)]
    pub base_experience: u32,
    #[serde(default)]
    pub stats: Vec<Stat>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Stat {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub base: u32,
}
