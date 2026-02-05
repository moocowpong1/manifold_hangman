#![allow(unused)]

use rand::{SeedableRng, Rng};
use rand_chacha::ChaCha12Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::io::Write;
use std::collections::HashMap;
use std::io::Take;
use std::iter;
use std::slice::RChunksMut;
use anyhow::{Result, Error};

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

// Track which letters in a word match the guess using bit flags
// The least significant bit corresponds to the start of the word.
type GuessSignature = u64;

// Checks which letters of a word match a guess letter and stores it as a signature.
fn word_signature(word: &String, guess: char) -> GuessSignature {
    let mut sig: GuessSignature = 0;
    for letter in word.chars().rev() {  // the rev() here saves me a headache
        sig *= 2;
        if letter == guess {sig += 1;}
    }
    sig
}

// returns an enum of bools indicating whether the signature matches the guess, 
// with the first bool returned referring to the first letter of the word.
fn decode_signature(mut sig: GuessSignature, num_letters: usize) -> impl Iterator<Item=bool> {
    let mut count = 0;
    return iter::from_fn(move || {
        if count == num_letters {
            None
        } else {
            count += 1;
            let m = sig % 2;    // no need to worry about signs because it's u64s 
            sig = (sig - m)/2;
            Some(m==1)
        }
    })
}

// prev_info is the information from before the guess, which should be updated by the guess signature.
// e.g. display_signature(12, "_A___T", 'B') = "_ABB_T"
fn display_signature(sig: GuessSignature, prev_info: &String, guess: char) -> String {
    let num_letters = prev_info.len();
    decode_signature(sig, num_letters).zip(prev_info.chars()).map(|(b, c)| if b {guess} else {c}).collect()
}

fn count_matches(sig: GuessSignature) -> u32 {
    sig.count_ones()
}

// This is the part that does the real work. Sorts the word list into buckets based on guess signature.
// Clones the words in word_list.
fn guess_buckets(word_list: &Vec<String>, guess: char) -> HashMap<GuessSignature, Vec<String>> {
    let mut buckets: HashMap<GuessSignature, Vec<String>> = HashMap::new();
    
    for word in word_list.iter() {
        let sig = word_signature(word, guess);
        if let Some(vec) = buckets.get_mut(&sig) {
            vec.push(word.clone());
        } else {
            buckets.insert(sig, vec![word.clone()]);
        }
    }

    buckets
}

fn display_guess_statistics(buckets: &HashMap<GuessSignature, Vec<String>>, prev_info: &String, guess: char) {
    let keys = buckets.keys();
    let mut pairs: Vec<(usize, GuessSignature)> = keys.map(|sig| (buckets.get(sig).unwrap().len(), *sig)).collect();

    // reverse sort to sort by word matches highest to lowest,
    // then whatever reverse signature sort is - it just needs to be consistent
    pairs.sort_by(|a, b| b.cmp(a));

    for (n, sig) in pairs {
        println!("{}: {}", display_signature(sig, prev_info, guess), n);
    }
}

// there's a library for this but it works with u32 weights and I want f64 weights
// we'll assume no NaN values or other problematic squirreliness. float rounding
// will be an issue but I think this handles it credibly.
// Assumes the option list is non-empty.
fn weighted_choice<T: Ord + Clone, R: Rng>(options: &mut Vec<(f64, T)>, rng: &mut R) -> T {
    // put these in a consistent order, lowest to highest weight.
    // partially for repeatability, partially for numerical stability issues:
    // if there's anything squirrely with the float math, we want to be subtracting
    // small from large until the very end, and land in the biggest bucket if there are issues.
    options.sort_by(|(w1, x1), (w2, x2)| {
        match w1.total_cmp(w2) {
            std::cmp::Ordering::Equal => x1.cmp(x2),
            ord => ord,
        }
    }); // lexicographical order using total_cmp

    let total_weight: f64 = options.iter().map(|t| t.0).sum();
    let mut dart: f64 = rng.random::<f64>() * total_weight;

    for (w, x) in options.iter() {
        dart -= w;
        if dart < 0.0 { return x.clone() }
    }

    //If we're still going, something squirrely happened with the float math and we should choose the most likely option
    options.last().unwrap().1.clone()
}

// Assumes the map of buckets is non-empty
fn choose_guess_outcome<R: Rng>(buckets: &HashMap<GuessSignature, Vec<String>>, settings: &Settings, rng: &mut R) -> GuessSignature {
    let mut options = Vec::new();

    for (sig, bucket) in buckets {
        let num_correct = count_matches(*sig);
        let bucket_size = bucket.len();
        let weight = (bucket_size as f64).powf(settings.evil_exponent) / settings.evil_factor.powf(num_correct as f64);
        options.push((weight, *sig));
    }

    // weighted choice sorts the options so we don't need to worry about inconsistent HashMap key orders.
    weighted_choice(&mut options, rng)
}

fn do_guess<R: Rng>(guess: char, word_list: &Vec<String>, settings: &Settings, rng: &mut R) -> (HashMap<GuessSignature, Vec<String>>, GuessSignature) {
    let buckets = guess_buckets(word_list, guess);
    let guess_result = choose_guess_outcome(&buckets, settings, rng);
    (buckets, guess_result)
}

pub fn initialize_game(settings: &Settings) -> History {
    let mut buffer = String::new();
    let mut rng_seed: u64 = 0;
    let mut letter_count: usize = 0;
    println!("No history file found, initializing a new game.");

    loop {
        buffer.clear();
        print!("Random seed? ");
        io::stdout().flush();
        let result = || -> Result<u64> {
            io::stdin().read_line(&mut buffer)?;
            println!("{}", &buffer);
            Ok(buffer.trim().parse()?)
        }();
        match result {
            Ok(s) => {rng_seed = s; break }
            Err(_) => println!("I couldn't read that, try again."),
        }
    }
    
    loop {
        buffer.clear();
        print!("Number of letters? ");
        io::stdout().flush();
        let result =  || -> Result<usize> {
            io::stdin().read_line(&mut buffer)?;
            println!("{}", &buffer);
            Ok(buffer.trim().parse()?)
        }();
        match result {
            Ok(l) => {
                if l < 1 || l > 64 {
                    println!("Number of letters must be between 1 and 64 inclusive.");
                } else {
                    letter_count = l; 
                    break 
                }
            }
            Err(_) => println!("I couldn't read that, try again."),
        }
    }

    History { rng_seed, letter_count: letter_count, guesses: Vec::new()}
}

fn save_history(history: &History, settings: &Settings) {
    print!("Saving history... ");
    io::stdout().flush();
    history.write_to_file(&settings.history_path);
    println!("Done!");
}

fn replay_history<R: Rng>(word_list: &mut Vec<String>, history: &History, settings: &Settings, rng: &mut R) -> String {
    word_list.retain(|word| word.len() == history.letter_count);
    if settings.verbose {println!("{} words of length {}", word_list.len(), history.letter_count);}
    let mut word_info = iter::repeat_n('_', history.letter_count).collect();

    for (n, &guess) in history.guesses.iter().enumerate() {
        print!("Guess #{}: {}  ", n, guess);
        let (mut buckets, guess_result) = do_guess(guess, word_list, settings, rng);
        word_info = display_signature(guess_result, &word_info, guess);
        *word_list = buckets.remove(&guess_result).unwrap();
        if(settings.verbose) { println!("Result: {}  Remaining Words: {}", &word_info, word_list.len()); }
    }

    word_info
}

fn read_guess() -> char {
    let mut buffer = String::new();
    let mut guess = '_';
    loop {
        buffer.clear();
        print!("Next guess? ");
        io::stdout().flush();
        let result = || -> Result<char> {
            io::stdin().read_line(&mut buffer)?;
            let c = buffer.chars().next().ok_or(Error::msg("Need at least one char"))?;
            if c.is_ascii_alphabetic() {
                Ok(c.to_ascii_uppercase())
            } else {
                Err(Error::msg("Need an ascii alphabetic character."))
            }
        }();
        match result {
            Ok(c) => {guess = c; break }
            Err(_) => println!("I couldn't read that, try again."),
        }
    }
    guess
}

pub fn play_game(mut word_list: Vec<String>, opt_history: Option<History>, settings: &Settings, rng_salt: u64) {
    let mut history = opt_history.unwrap_or_else(|| initialize_game(settings));
    let mut rng = ChaCha12Rng::seed_from_u64(rng_salt ^ history.rng_seed);
    let mut word_info = replay_history(&mut word_list, &history, settings, &mut rng);
    save_history(&history, settings);
    loop {
        let guess = read_guess();
        let (mut buckets, guess_result) = do_guess(guess, &word_list, settings, &mut rng);
        history.guesses.push(guess);

        if settings.verbose { display_guess_statistics(&buckets, &word_info, guess); }
        word_info = display_signature(guess_result, &word_info, guess);
        word_list = buckets.remove(&guess_result).unwrap();
        if(settings.verbose) { 
            println!("Result: {}  Remaining Words: {}", &word_info, word_list.len());
            println!("Guesses so far: {}", history.guesses.iter().collect::<String>())
        }
        save_history(&history, settings); 
        if !word_info.contains('_') { break }
    }
    println!("Winner! The word was {}", &word_info);
}
