use crate::util::std_dev;
use crate::TICK_RATE_MS;
use chrono::prelude::*;
use directories::ProjectDirs;
use itertools::Itertools;
use ratatui::text::Text;
use std::cell::OnceCell;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::{char, collections::HashMap, time::SystemTime};

#[derive(Clone, Debug, Copy, PartialEq)]
pub enum Outcome {
    Correct,
    Incorrect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub char: char,
    pub outcome: Outcome,
    pub timestamp: SystemTime,
}

/// represents a test being displayed to the user
pub struct Thok<'a> {
    pub prompt: String,
    pub input: Vec<Input>,
    pub raw_coords: Vec<(f64, f64)>,
    pub wpm_coords: Vec<(f64, f64)>,
    pub cursor_pos: usize,
    pub started_at: Option<SystemTime>,
    pub seconds_remaining: Option<f64>,
    pub number_of_secs: Option<f64>,
    pub number_of_words: usize,
    pub wpm: f64,
    pub accuracy: f64,
    pub std_dev: f64,
    pub pace: Option<f64>,
    pub death_mode: bool,
    pub skull_cache: OnceCell<Text<'a>>,
    pub tabbed: bool,
}

impl Thok<'_> {
    pub fn new(
        prompt: String,
        number_of_words: usize,
        number_of_secs: Option<f64>,
        pace: Option<f64>,
        death_mode: bool,
    ) -> Self {
        Self {
            prompt,
            input: vec![],
            raw_coords: vec![],
            wpm_coords: vec![],
            cursor_pos: 0,
            started_at: None,
            number_of_secs,
            number_of_words,
            seconds_remaining: number_of_secs,
            wpm: 0.0,
            accuracy: 0.0,
            std_dev: 0.0,
            pace,
            death_mode,
            skull_cache: OnceCell::new(),
            tabbed: false,
        }
    }

    pub fn on_tick(&mut self) {
        if let Some(v) = self.seconds_remaining {
            self.seconds_remaining = Some(v - (TICK_RATE_MS as f64 / 1000_f64));
        }
    }

    pub fn get_expected_char(&self, idx: usize) -> char {
        self.prompt.chars().nth(idx).unwrap()
    }

    pub fn increment_cursor(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.cursor_pos += 1;
        }
    }

    pub fn decrement_cursor(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub fn calc_results(&mut self) {
        let correct_chars = self
            .input
            .clone()
            .into_iter()
            .filter(|i| i.outcome == Outcome::Correct)
            .collect::<Vec<Input>>();

        let elapsed_secs =
            self.started_at.unwrap().elapsed().unwrap().as_millis() as f64;

        let whole_second_limit = elapsed_secs.floor();

        let correct_chars_per_sec: Vec<(f64, f64)> = correct_chars
            .clone()
            .into_iter()
            .fold(HashMap::new(), |mut map, i| {
                let mut num_secs = i
                    .timestamp
                    .duration_since(self.started_at.unwrap())
                    .unwrap()
                    .as_secs_f64();

                if num_secs == 0.0 {
                    num_secs = 1.;
                } else if num_secs.ceil() <= whole_second_limit {
                    if num_secs > 0. && num_secs < 1. {
                        // this accounts for the initiated keypress at 0.000
                        num_secs = 1.;
                    } else {
                        num_secs = num_secs.ceil()
                    }
                } else {
                    num_secs = elapsed_secs;
                }

                *map.entry(num_secs.to_string()).or_insert(0) += 1;
                map
            })
            .into_iter()
            .map(|(k, v)| (k.parse::<f64>().unwrap(), v as f64))
            .sorted_by(|a, b| a.partial_cmp(b).unwrap())
            .collect();

        let correct_chars_at_whole_sec_intervals = correct_chars_per_sec
            .iter()
            .enumerate()
            .filter(|&(i, _)| i < correct_chars_per_sec.len() - 1)
            .map(|(_, x)| x.1)
            .collect::<Vec<f64>>();

        if !correct_chars_at_whole_sec_intervals.is_empty() {
            self.std_dev =
                std_dev(&correct_chars_at_whole_sec_intervals).unwrap();
        } else {
            self.std_dev = 0.0;
        }

        let mut correct_chars_pressed_until_now = 0.0;

        for x in correct_chars_per_sec {
            correct_chars_pressed_until_now += x.1;
            self.wpm_coords.push((
                x.0,
                ((60.00 / x.0) * correct_chars_pressed_until_now) / 5.0,
            ))
        }

        let correct_words = self
            .input
            .clone()
            .split(|i| i.char == ' ')
            .filter(|&s| !s.iter().any(|i| i.outcome == Outcome::Incorrect))
            .count();

        self.wpm = (correct_words as f64
            / (self.started_at.unwrap().elapsed().unwrap().as_secs_f64()
                / 60.0))
            .ceil();
        self.accuracy =
            ((correct_chars.len() as f64 / self.input.len() as f64) * 100.0)
                .round();

        let _ = self.save_results();
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.input.remove(self.cursor_pos - 1);
            self.decrement_cursor();
        }
    }

    pub fn word_backspace(&mut self) {
        if let Some(Input { char: ' ', .. }) = self.input.last() {
            self.input.remove(self.cursor_pos - 1);
            self.decrement_cursor();
        }
        while self.input.last().is_some_and(|i| i.char != ' ') {
            self.input.remove(self.cursor_pos - 1);
            self.decrement_cursor();
        }
    }

    pub fn start(&mut self) {
        self.started_at = Some(SystemTime::now());
    }

    pub fn write(&mut self, c: char) {
        let idx = self.input.len();
        if idx == 0 && self.started_at.is_none() {
            self.start();
        }

        let outcome = if c == self.get_expected_char(idx) {
            Outcome::Correct
        } else {
            Outcome::Incorrect
        };

        self.input.insert(
            self.cursor_pos,
            Input {
                char: c,
                outcome,
                timestamp: SystemTime::now(),
            },
        );
        self.increment_cursor();
    }

    pub fn has_started(&self) -> bool {
        self.started_at.is_some()
    }

    pub fn has_finished(&self) -> bool {
        let finished_prompt = self.input.len() == self.prompt.len();
        let out_of_time = self.seconds_remaining.is_some()
            && self.seconds_remaining.unwrap() <= 0.0;
        let is_fatal_error = self.fatal_error();

        finished_prompt || out_of_time || is_fatal_error
    }

    pub fn save_results(&self) -> io::Result<()> {
        if let Some(proj_dirs) = ProjectDirs::from("", "", "thokr") {
            let config_dir = proj_dirs.config_dir();
            let log_path = config_dir.join("log.csv");

            std::fs::create_dir_all(config_dir)?;

            // If the config file doesn't exist, we need to emit a header
            let needs_header = !log_path.exists();

            let mut log_file = OpenOptions::new()
                .append(true)
                .create(true)
                .open(log_path)?;

            if needs_header {
                writeln!(
                    log_file,
                    "date,num_words,num_secs,elapsed_secs,wpm,accuracy,std_dev"
                )?;
            }

            let elapsed_secs =
                self.started_at.unwrap().elapsed().unwrap().as_secs_f64();

            writeln!(
                log_file,
                "{},{},{},{:.2},{},{},{:.2}",
                Local::now().format("%c"),
                self.number_of_words,
                self.number_of_secs
                    .map_or(String::from(""), |ns| format!("{:.2}", ns)),
                elapsed_secs,
                self.wpm, // already rounded, no need to round to two decimal places
                self.accuracy, // already rounded, no need to round to two decimal places
                self.std_dev,
            )?;
        }

        Ok(())
    }

    pub fn fatal_error(&self) -> bool {
        self.death_mode
            && self.input.iter().any(|i| i.outcome == Outcome::Incorrect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! build_input {
        ($s:expr) => {
            $s.chars()
                .map(|c| Input {
                    char: c,
                    outcome: Outcome::Correct,
                    timestamp: SystemTime::now(),
                })
                .collect::<Vec<_>>()
        };
        ($i:expr, $g:expr) => {
            $i.chars().zip($g.chars()).map(|(c, g)| Input {
                char: c,
                outcome: if c == g {
                    Outcome::Correct
                } else {
                    Outcome::Incorrect
                },
                timestamp: SystemTime::now(),
            }).collect::<Vec<_>>()
        };
    }
    macro_rules! assert_within {
        ($first:expr, $second:expr, $maxdelta:expr) => {
            if ($first - $second).abs() > $maxdelta {
                panic!(
                    "assert_within failed: {} is not close enough to {}",
                    $first, $second
                );
            }
        };
    }
    #[test]
    fn test_wpm() {
        use std::time::Duration;
        let mut thok = Thok {
            prompt: "one two three".to_string(),
            input: build_input!("one two three"),
            raw_coords: Vec::new(),
            wpm_coords: Vec::new(),
            cursor_pos: 13,
            started_at: Some(SystemTime::now() - Duration::from_secs(1)),
            seconds_remaining: None,
            number_of_secs: None,
            number_of_words: 3,
            wpm: 0.,
            accuracy: 0.,
            std_dev: 0.,
            pace: None,
            death_mode: false,
            skull_cache: OnceCell::new(),
            tabbed: false,
        };

        thok.calc_results();
        assert_within!(thok.wpm, 180., 5.);

        thok.started_at = Some(SystemTime::now() - Duration::from_secs(3));
        thok.calc_results();
        assert_within!(thok.wpm, 60., 5.);
    }
    #[test]
    fn test_accuracy() {
        use std::time::Duration;

        let mut thok = Thok {
            prompt: "one two three".to_string(),
            input: build_input!("one two thrdd", "one two three"),
            raw_coords: Vec::new(),
            wpm_coords: Vec::new(),
            cursor_pos: 13,
            started_at: Some(SystemTime::now() - Duration::from_secs(1)),
            seconds_remaining: None,
            number_of_secs: None,
            number_of_words: 3,
            wpm: 0.,
            accuracy: 0.,
            std_dev: 0.,
            pace: None,
            death_mode: false,
            skull_cache: OnceCell::new(),
            tabbed: false,
        };

        thok.calc_results();
        assert_within!(thok.accuracy, 85., 5.);

        thok.input = build_input!("one two three");
        thok.calc_results();
        assert_within!(thok.accuracy, 100., 5.);
    }
    #[test]
    fn test_word_backspace() {
        use std::time::Duration;

        let mut thok = Thok {
            prompt: "one two three four".to_string(),
            input: build_input!("one two three four"),
            raw_coords: Vec::new(),
            wpm_coords: Vec::new(),
            cursor_pos: 18,
            started_at: Some(SystemTime::now() - Duration::from_secs(1)),
            seconds_remaining: None,
            number_of_secs: None,
            number_of_words: 4,
            wpm: 0.,
            accuracy: 0.,
            std_dev: 0.,
            pace: None,
            death_mode: false,
            skull_cache: OnceCell::new(),
            tabbed: false,
        };

        thok.word_backspace();
        let input = thok
            .input
            .clone()
            .into_iter()
            .map(|i| i.char)
            .collect::<String>();
        assert_eq!(input, "one two three ");

        thok.word_backspace();
        let input = thok
            .input
            .clone()
            .into_iter()
            .map(|i| i.char)
            .collect::<String>();
        assert_eq!(input, "one two ");
    }
}
