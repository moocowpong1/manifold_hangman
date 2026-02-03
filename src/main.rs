use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;

mod hangman;
use hangman::{Settings, History};

const SETTINGS_PATH: &str = "settings.toml";


#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct SeedSalt {
    salt: u64,
}

impl SeedSalt {
    fn from_file(path: &str) -> u64 {
        let content = fs::read_to_string(path)
            .expect("Failed to read seed file");
        let sd: SeedSalt = toml::from_str(&content)
            .expect("Failed to parse seed file");

        sd.salt
    }
}

impl Settings {
    /// Load settings from a TOML file, panicking on error
    pub fn from_file(path: &str) -> Self {
        let content = fs::read_to_string(path)
            .expect("Failed to read settings file");
        toml::from_str(&content)
            .expect("Failed to parse settings file")
    }
}

impl History {
    pub fn from_file(path: &str) -> Option<Self> {
        let content = fs::read_to_string(path).ok();
        content.map(|c| toml::from_str(&c)
            .expect("Failed to parse history file"))
    }
}

fn read_word_list(word_path: &str, exclusions_path: &str) -> Vec<String> {
    let alphabetic_regex = Regex::new("^[a-zA-Z]+$").unwrap();
    let exclusions_content = fs::read_to_string(exclusions_path)
        .expect("Failed to read exclusions file");
    let mut exclusions = Vec::new();
    for ex in exclusions_content.lines() {
        if !alphabetic_regex.is_match(ex) {continue;}
        exclusions.push(ex);
    }

    let words_content = fs::read_to_string(word_path)
        .expect("Failed to read word list file");
    let mut words = Vec::new();
    for word in words_content.lines() {
        if !alphabetic_regex.is_match(word) {continue;}
        if exclusions.contains(&word) {continue;}   //skip excluded words
        words.push(word.to_string().to_uppercase())
    }
    words.sort();
    words.dedup();

    words
}

fn main() {
    let settings = Settings::from_file(SETTINGS_PATH);
    let word_list = read_word_list(&settings.word_list_path, &settings.word_list_path);
    let rng_salt = SeedSalt::from_file(&settings.salt_file_path);
    let history = History::from_file(&settings.history_path);
    hangman::play_game(word_list, history, &settings, rng_salt)
}
