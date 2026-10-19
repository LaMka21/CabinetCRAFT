//! Jeden jazyk: texty zo súboru `locales/<kód>.toml` ako plochá mapa kľúčov.
//!
//! Tabuľky TOML tvoria kľúč cez bodky (`[ribbon] return = "…"` → `ribbon.return`).
//! Tabuľka, ktorá má iba kategórie množného čísla a medzi nimi `other`, je jedna správa
//! s tvarmi podľa čísla (`parts = { one = "{count} diel", few = …, other = … }`).
//! Sekcia `[meta]` nie je text: `name` (meno jazyka v prepínači) a `plural` (pravidlo).

use std::collections::{BTreeMap, HashMap};

use crate::plural::Category;

/// Jedna správa: text alebo tvary podľa množného čísla.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Text(String),
    Plural(BTreeMap<Category, String>),
}

impl Message {
    /// Text pre kategóriu (chýbajúca kategória → `other`).
    pub fn text_for(&self, category: Option<Category>) -> &str {
        match self {
            Message::Text(t) => t,
            Message::Plural(forms) => category
                .and_then(|c| forms.get(&c))
                .or_else(|| forms.get(&Category::Other))
                .or_else(|| forms.values().next())
                .map_or("", String::as_str),
        }
    }

    /// Všetky texty správy (na kontrolu parametrov).
    pub fn texts(&self) -> Vec<&str> {
        match self {
            Message::Text(t) => vec![t.as_str()],
            Message::Plural(forms) => forms.values().map(String::as_str).collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.texts().iter().all(|t| t.trim().is_empty())
    }
}

/// Chyba čítania súboru prekladu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Texty jedného jazyka.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    /// Kód jazyka podľa mena súboru (`sk`, `en`, `pt-BR`).
    pub code: String,
    /// Meno jazyka v prepínači (`[meta] name`), inak kód.
    pub name: String,
    /// Pravidlo množného čísla (`[meta] plural`), inak kód.
    pub plural: String,
    messages: HashMap<String, Message>,
    /// Hodnoty, ktoré nie sú text ani tvary čísla (pre kontrolu `cargo xtask i18n`).
    pub warnings: Vec<String>,
}

impl Catalog {
    /// Prázdny katalóg (každý kľúč chýba).
    pub fn empty(code: &str) -> Catalog {
        Catalog {
            code: code.to_owned(),
            name: code.to_owned(),
            plural: code.to_owned(),
            ..Catalog::default()
        }
    }

    /// Prečíta súbor prekladu. Prázdne texty sa vynechajú (kľúč chýba → záloha).
    pub fn parse(code: &str, text: &str) -> Result<Catalog, ParseError> {
        let table: toml::Table = text.parse().map_err(|e: toml::de::Error| ParseError {
            message: e.message().to_owned(),
        })?;
        let mut catalog = Catalog::empty(code);
        for (key, value) in &table {
            if key == "meta" {
                catalog.read_meta(value);
                continue;
            }
            catalog.flatten(key, value);
        }
        Ok(catalog)
    }

    fn read_meta(&mut self, value: &toml::Value) {
        let Some(meta) = value.as_table() else {
            self.warnings.push("`meta` nie je tabuľka".into());
            return;
        };
        if let Some(name) = meta.get("name").and_then(toml::Value::as_str) {
            if !name.trim().is_empty() {
                self.name = name.trim().to_owned();
            }
        }
        if let Some(rule) = meta.get("plural").and_then(toml::Value::as_str) {
            if !rule.trim().is_empty() {
                self.plural = rule.trim().to_owned();
            }
        }
    }

    fn flatten(&mut self, key: &str, value: &toml::Value) {
        match value {
            toml::Value::String(s) => self.insert(key, Message::Text(s.clone())),
            toml::Value::Table(t) => {
                if let Some(forms) = plural_forms(t) {
                    self.insert(key, Message::Plural(forms));
                } else {
                    for (k, v) in t {
                        self.flatten(&format!("{key}.{k}"), v);
                    }
                }
            }
            _ => self
                .warnings
                .push(format!("`{key}`: hodnota nie je text v úvodzovkách")),
        }
    }

    fn insert(&mut self, key: &str, message: Message) {
        if !message.is_empty() {
            self.messages.insert(key.to_owned(), message);
        }
    }

    /// Prepíše texty textami z `other` (súbor používateľa nad vstavaným).
    pub fn overlay(&mut self, other: Catalog) {
        if other.name != other.code {
            self.name = other.name;
        }
        if other.plural != other.code {
            self.plural = other.plural;
        }
        self.messages.extend(other.messages);
        self.warnings.extend(other.warnings);
    }

    pub fn get(&self, key: &str) -> Option<&Message> {
        self.messages.get(key)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.messages.contains_key(key)
    }

    /// Kľúče v abecednom poradí.
    pub fn keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.messages.keys().map(String::as_str).collect();
        keys.sort_unstable();
        keys
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

/// Tvary množného čísla, ak tabuľka má iba kategórie a medzi nimi `other`.
fn plural_forms(t: &toml::Table) -> Option<BTreeMap<Category, String>> {
    if !t.contains_key("other") {
        return None;
    }
    let mut forms = BTreeMap::new();
    for (k, v) in t {
        let category = Category::from_name(k)?;
        forms.insert(category, v.as_str()?.to_owned());
    }
    Some(forms)
}

/// Mená parametrov `{meno}` v texte (bez `{{`).
pub fn placeholders(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '{' if chars.peek().map(|(_, c)| *c) == Some('{') => {
                chars.next();
            }
            '{' => {
                let rest = &text[i + 1..];
                if let Some(end) = rest.find('}') {
                    let name = &rest[..end];
                    if is_name(name) {
                        names.push(name.to_owned());
                    }
                }
            }
            _ => {}
        }
    }
    names.sort();
    names.dedup();
    names
}

pub(crate) fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[meta]
name = "Slovenčina"

[ribbon]
return = "Návrat"
empty = ""
number = 5

[ribbon.view]
zoom = "Zoom"

[parts]
count = { one = "{count} diel", few = "{count} diely", other = "{count} dielov" }

[odd]
one = "toto nie je množné číslo, chýba other"
"#;

    #[test]
    fn flattens_tables_and_plurals() {
        let c = Catalog::parse("sk", SAMPLE).unwrap();
        assert_eq!(c.name, "Slovenčina");
        assert_eq!(
            c.get("ribbon.return"),
            Some(&Message::Text("Návrat".into()))
        );
        assert_eq!(
            c.get("ribbon.view.zoom"),
            Some(&Message::Text("Zoom".into()))
        );
        assert!(!c.contains("ribbon.empty"), "prázdny text = chýbajúci kľúč");
        assert!(c.contains("odd.one"));
        assert_eq!(c.warnings.len(), 1);
        let m = c.get("parts.count").unwrap();
        assert_eq!(m.text_for(Some(Category::Few)), "{count} diely");
        assert_eq!(m.text_for(Some(Category::Many)), "{count} dielov");
        assert_eq!(c.keys().len(), 4);
    }

    #[test]
    fn broken_file_is_error() {
        assert!(Catalog::parse("xx", "a = \"neukončený").is_err());
        assert!(Catalog::parse("xx", "[a]\n[a]").is_err());
    }

    #[test]
    fn finds_placeholders() {
        assert_eq!(
            placeholders("{count} z {all}, {{x}} {count}"),
            ["all", "count"]
        );
        assert!(placeholders("{ nie }{").is_empty());
    }
}
