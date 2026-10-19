//! Register príkazov. Každá akcia používateľa je jeden [`CommandSpec`].

mod cabinet;
mod catalog;
mod drawings;
mod edit;
mod export;
mod front;
mod inspect;
mod labels;
mod project;
mod room;
mod standard;

use cc_i18n::tr;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::{EngineError, Session};

/// Čo príkaz robí s dokumentom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Iba číta (môže zmeniť výber).
    Read,
    /// Mení projekt alebo štandard: uloží krok späť, skontroluje vstup, prepočíta model.
    Edit,
    /// Súbory a história (otvorenie, uloženie, späť/znova): prepočíta model, krok späť nevytvára.
    Other,
}

pub struct CommandSpec {
    /// `<objekt>.<sloveso>`, napr. `cabinet.add`.
    pub id: &'static str,
    /// Príkaz má miesto v ponuke (cesta je v prekladoch `command_menus.<id>`).
    pub menu: bool,
    pub shortcut: Option<&'static str>,
    /// Tvar parametrov v JSON (pre agenta a dokumentáciu).
    pub params: &'static str,
    pub effect: Effect,
    /// `Err(dôvod)`: príkaz teraz nejde spustiť.
    pub enabled: fn(&Session) -> Result<(), String>,
    pub run: fn(&mut Session, Value) -> Result<Value, EngineError>,
}

/// Názov príkazu v zvolenom jazyku (`commands.<id>` v `locales/*.toml`).
pub fn label_of(id: &str) -> String {
    cc_i18n::tr(&format!("commands.{id}"))
}

impl CommandSpec {
    /// Názov pre ponuku a históriu krokov v zvolenom jazyku.
    pub fn label(&self) -> String {
        label_of(self.id)
    }

    /// Cesta v ponuke v zvolenom jazyku, napr. `Skrinka/Pridať`.
    pub fn menu_path(&self) -> Option<String> {
        self.menu
            .then(|| cc_i18n::tr(&format!("command_menus.{}", self.id)))
    }

    /// Popis príkazu pre `engine.commands`.
    pub fn describe(&self, session: &Session) -> Value {
        let enabled = (self.enabled)(session);
        json!({
            "id": self.id,
            "label": self.label(),
            "menu": self.menu_path(),
            "shortcut": self.shortcut,
            "params": self.params,
            "effect": match self.effect {
                Effect::Read => "read",
                Effect::Edit => "edit",
                Effect::Other => "other",
            },
            "enabled": enabled.is_ok(),
            "disabledReason": enabled.err(),
        })
    }
}

static COMMANDS: &[&[CommandSpec]] = &[
    project::COMMANDS,
    cabinet::COMMANDS,
    front::COMMANDS,
    catalog::COMMANDS,
    room::COMMANDS,
    standard::COMMANDS,
    edit::COMMANDS,
    export::COMMANDS,
    drawings::COMMANDS,
    labels::COMMANDS,
    inspect::COMMANDS,
];

/// Všetky príkazy v poradí registra.
pub fn commands() -> impl Iterator<Item = &'static CommandSpec> {
    COMMANDS.iter().flat_map(|group| group.iter())
}

pub fn find(id: &str) -> Option<&'static CommandSpec> {
    commands().find(|c| c.id == id)
}

// ----- pomôcky pre príkazy -----

pub(crate) fn always(_: &Session) -> Result<(), String> {
    Ok(())
}

pub(crate) fn has_doc(s: &Session) -> Result<(), String> {
    s.doc()
        .map(|_| ())
        .ok_or_else(|| tr!("engine.errors.no_project"))
}

/// Parametre príkazu `command` z JSON. Neznáme polia sú chyba.
pub(crate) fn parse<T: DeserializeOwned>(command: &str, params: Value) -> Result<T, EngineError> {
    serde_json::from_value(params).map_err(|e| EngineError::Params {
        command: command.to_owned(),
        message: e.to_string(),
    })
}

/// Konečné kladné číslo (rozmer).
pub(crate) fn positive(command: &str, name: &str, v: f64) -> Result<f64, EngineError> {
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(EngineError::Params {
            command: command.to_owned(),
            message: tr!("engine.errors.not_positive", name = name, value = v),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique_and_well_formed() {
        let mut seen = HashSet::new();
        for c in commands() {
            assert!(seen.insert(c.id), "príkaz {} je v registri dvakrát", c.id);
            let (object, verb) = c.id.split_once('.').unwrap();
            assert!(!object.is_empty() && !verb.is_empty(), "{}", c.id);
        }
    }

    /// Každý príkaz má názov (a príkaz v ponuke cestu) v slovenčine aj angličtine.
    #[test]
    fn every_command_has_translated_label() {
        for (code, text) in cc_i18n::BUILTIN {
            let catalog = cc_i18n::Catalog::parse(code, text).unwrap();
            for c in commands() {
                let key = format!("commands.{}", c.id);
                assert!(catalog.contains(&key), "{code}.toml: chýba {key}");
                let menu = format!("command_menus.{}", c.id);
                assert_eq!(catalog.contains(&menu), c.menu, "{code}.toml: {menu}");
            }
        }
    }
}
