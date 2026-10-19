//! Späť/znova cez snímky projektu a štandardu (ADR 0002).

use cc_model::{ProjectFile, Standard};

use crate::Document;

/// Najviac krokov späť.
pub const LIMIT: usize = 200;

/// Stav dokumentu, ktorý sa dá vrátiť.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub project: ProjectFile,
    pub standard: Standard,
    pub dirty: bool,
    pub standard_dirty: bool,
}

impl State {
    pub fn capture(doc: &Document) -> State {
        State {
            project: doc.project.clone(),
            standard: doc.standard.clone(),
            dirty: doc.dirty,
            standard_dirty: doc.standard_dirty,
        }
    }

    pub fn restore(self, doc: &mut Document) {
        doc.project = self.project;
        doc.standard = self.standard;
        doc.dirty = self.dirty;
        doc.standard_dirty = self.standard_dirty;
    }

    /// Rovnaký obsah (príznaky neuložených zmien sa neporovnávajú).
    pub fn same_content(&self, doc: &Document) -> bool {
        self.project == doc.project && self.standard == doc.standard
    }
}

#[derive(Debug, Clone)]
struct Entry {
    /// Príkaz kroku (`cabinet.add`); názov sa prekladá až pri zobrazení.
    label: String,
    /// Kroky s rovnakým kľúčom idúce po sebe sa zlúčia do jedného (ťahanie posuvníka).
    key: Option<String>,
    before: State,
}

#[derive(Debug, Clone, Default)]
pub struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
}

impl History {
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Zapíše krok. Ak má rovnaký kľúč ako posledný krok, zlúči sa s ním
    /// (ostane pôvodný stav pred prvým krokom).
    pub fn push(&mut self, label: &str, key: Option<String>, before: State) {
        self.redo.clear();
        if let (Some(k), Some(last)) = (&key, self.undo.last()) {
            if last.key.as_ref() == Some(k) {
                return;
            }
        }
        self.undo.push(Entry {
            label: label.to_owned(),
            key,
            before,
        });
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
    }

    /// Ukončí zlučovanie: ďalší krok s rovnakým kľúčom bude samostatný.
    pub fn seal(&mut self) {
        if let Some(last) = self.undo.last_mut() {
            last.key = None;
        }
    }

    pub fn undo(&mut self, current: State) -> Option<(String, State)> {
        let entry = self.undo.pop()?;
        self.redo.push(Entry {
            label: entry.label.clone(),
            key: None,
            before: current,
        });
        Some((entry.label, entry.before))
    }

    pub fn redo(&mut self, current: State) -> Option<(String, State)> {
        let entry = self.redo.pop()?;
        self.undo.push(Entry {
            label: entry.label.clone(),
            key: None,
            before: current,
        });
        Some((entry.label, entry.before))
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|e| e.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|e| e.label.as_str())
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
}
