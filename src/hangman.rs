#![allow(unused)]

use rand_core::{SeedableRng, Rng};
use rand_chacha::ChaCha12Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub evil_exponent: f64,
    pub evil_factor: f64,
    pub word_list_path: String,
    pub exclusions_list_path: String,
    pub salt_file_path: String,
    pub history_path: String,
    pub verbose: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct History {
    pub rng_seed: u64,
    pub letter_count: usize,
    pub guesses: Vec<char>,
}

impl History {
    pub fn write_to_file(&self, path: &str) {
        let history_string = toml::to_string_pretty(self)
            .expect("Failed to serialize history");
        fs::write(path, history_string)
            .expect("Failed to write history file");
    }
}

pub fn initialize_game(settings: &Settings) -> History {
    let mut buffer = String::new();
    let mut rng_seed: u64 = 0;
    let mut letter_count: usize = 0;
    println!("No history file found, initializing a new game.");

    loop {
        buffer.clear();
        println!("Random seed?");
        let result = || -> Result<u64> {
            io::stdin().read_line(&mut buffer)?;
            Ok(buffer.parse()?)
        }();
        match result {
            Ok(s) => {rng_seed = s; break }
            Err(_) => println!("I couldn't read that, try again."),
        }
    }
    
    loop {
        buffer.clear();
        println!("Number of letters?");
        let result =  || -> Result<usize> {
            io::stdin().read_line(&mut buffer)?;
            Ok(buffer.parse()?)
        }();
        match result {
            Ok(l) => {letter_count = l; break }
            Err(_) => println!("I couldn't read that, try again."),
        }
    }

    History { rng_seed, letter_count: letter_count, guesses: Vec::new()}
}

pub fn replay_history(word_list: &mut Vec<String>, history: &History, settings: &Settings, rng: &mut ChaCha12Rng) {
    word_list.retain(|word| word.len() == history.letter_count);
    if settings.verbose {println!("{} words of length {}", word_list.len(), history.letter_count);}

    for (n, guess) in history.guesses.iter().enumerate() {

    }
}

pub fn play_game(mut word_list: Vec<String>, opt_history: Option<History>, settings: &Settings, rng_salt: u64) {
    let mut history = opt_history.unwrap_or(initialize_game(settings));
    let mut rng = ChaCha12Rng::seed_from_u64(rng_salt ^ history.rng_seed);
    replay_history(&mut word_list, &history, settings, &mut rng);
}
