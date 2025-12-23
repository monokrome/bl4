use anyhow::Result;
use rand::seq::SliceRandom;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Rule {
    pub rule_type: String,
    pub position: Option<usize>,
    pub value: Option<u8>,
    pub length: Option<usize>,
}

impl Rule {
    /// Parse a rule from a line like "byte_equals 0 33" or "min_length 30"
    pub fn from_line(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let rule_type = parts[0].to_string();
        match rule_type.as_str() {
            "byte_equals" => {
                // byte_equals <position> <value>
                let position = parts.get(1)?.parse().ok();
                let value = parts.get(2)?.parse().ok();
                Some(Rule {
                    rule_type,
                    position,
                    value,
                    length: None,
                })
            }
            "min_length" => {
                // min_length <length>
                let length = parts.get(1)?.parse().ok();
                Some(Rule {
                    rule_type,
                    position: None,
                    value: None,
                    length,
                })
            }
            "max_length" => {
                let length = parts.get(1)?.parse().ok();
                Some(Rule {
                    rule_type,
                    position: None,
                    value: None,
                    length,
                })
            }
            _ => None,
        }
    }

    pub fn matches(&self, record: &[u8]) -> bool {
        match self.rule_type.as_str() {
            "byte_equals" => {
                let pos = self.position.unwrap_or(0);
                let val = self.value.unwrap_or(0);
                record.get(pos) == Some(&val)
            }
            "min_length" => {
                let len = self.length.unwrap_or(0);
                record.len() >= len
            }
            "max_length" => {
                let len = self.length.unwrap_or(usize::MAX);
                record.len() <= len
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PresetRules {
    pub name: String,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub presets: Vec<PresetRules>,
}

impl Config {
    pub async fn load() -> Result<Self> {
        let home = std::env::var("HOME").unwrap_or_default();
        let preset_dir = format!("{}/.config/linewise/presets", home);

        let mut config = Config::default();

        // Scan preset directory for .lwpreset files
        if let Ok(entries) = fs::read_dir(&preset_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "lwpreset").unwrap_or(false) {
                    if let Some(preset_rules) = Self::load_preset_rules(&path) {
                        if !preset_rules.rules.is_empty() {
                            config.presets.push(preset_rules);
                        }
                    }
                }
            }
        }

        Ok(config)
    }

    fn load_preset_rules(path: &Path) -> Option<PresetRules> {
        let content = fs::read_to_string(path).ok()?;
        let name = path.file_stem()?.to_string_lossy().to_string();

        let mut rules = Vec::new();
        let mut in_rules_section = false;

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments (unless in rules section)
            if line.is_empty() {
                continue;
            }

            if line == "@rules" {
                in_rules_section = true;
                continue;
            }

            if in_rules_section && !line.starts_with('#') {
                if let Some(rule) = Rule::from_line(line) {
                    rules.push(rule);
                }
            }
        }

        Some(PresetRules { name, rules })
    }

    pub fn detect_preset(&self, records: &[Vec<u8>], sample_size: usize) -> Option<String> {
        if records.is_empty() || self.presets.is_empty() {
            return None;
        }

        let mut rng = rand::thread_rng();
        let samples: Vec<&Vec<u8>> = if records.len() <= sample_size {
            records.iter().collect()
        } else {
            records.choose_multiple(&mut rng, sample_size).collect()
        };

        let mut best_match: Option<(String, usize)> = None;

        for preset in &self.presets {
            if preset.rules.is_empty() {
                continue;
            }

            let matches = samples
                .iter()
                .filter(|record| preset.rules.iter().all(|rule| rule.matches(record)))
                .count();

            let threshold = (samples.len() * 80) / 100;
            if matches >= threshold {
                match &best_match {
                    None => best_match = Some((preset.name.clone(), matches)),
                    Some((_, best_count)) if matches > *best_count => {
                        best_match = Some((preset.name.clone(), matches));
                    }
                    _ => {}
                }
            }
        }

        best_match.map(|(name, _)| name)
    }
}
