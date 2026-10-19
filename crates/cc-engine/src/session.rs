//! Otvorený projekt, postavený model a vykonávanie príkazov.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::Arc;

use cc_pipeline::Built;
use serde::Serialize;
use serde_json::Value;

use cc_i18n::tr;

use crate::cmd::{self, Effect};
use crate::history::{History, State};
use crate::{Document, EngineError, MAX_CABINETS};

/// Čo je vybrané v okne (skrinka, diel). Mení sa príkazom `selection.set`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Selection {
    pub cabinet: Option<String>,
    pub part: Option<String>,
}

#[derive(Default)]
pub struct Session {
    doc: Option<Document>,
    built: Option<Result<Arc<Built>, String>>,
    pub(crate) history: History,
    pub(crate) selection: Selection,
    version: u64,
    /// Príkaz nastavil príznaky `dirty`/`standard_dirty` sám.
    pub(crate) flags_set: bool,
}

impl Session {
    /// Prázdna session bez projektu.
    pub fn new() -> Session {
        Session::default()
    }

    /// Session s otvoreným projektom.
    pub fn open(path: &Path) -> Result<Session, String> {
        let mut s = Session::new();
        s.set_document(Document::open(path)?);
        Ok(s)
    }

    pub fn doc(&self) -> Option<&Document> {
        self.doc.as_ref()
    }

    /// Posledný postavený model, alebo chyba stavby. `None`: nie je otvorený projekt.
    pub fn built(&self) -> Option<&Result<Arc<Built>, String>> {
        self.built.as_ref()
    }

    /// Postavený model, ak sa podaril.
    pub fn model(&self) -> Option<Arc<Built>> {
        self.built.as_ref()?.as_ref().ok().cloned()
    }

    /// Zvýši sa pri každej zmene dokumentu, modelu alebo výberu (okno podľa neho prekreslí).
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    /// Názov kroku, ktorý vráti Späť (v zvolenom jazyku).
    pub fn undo_label(&self) -> Option<String> {
        self.history.undo_label().map(cmd::label_of)
    }

    /// Názov kroku, ktorý vykoná Znova (v zvolenom jazyku).
    pub fn redo_label(&self) -> Option<String> {
        self.history.redo_label().map(cmd::label_of)
    }

    /// Ukončí zlučovanie krokov (napr. po pustení posuvníka).
    pub fn seal_history(&mut self) {
        self.history.seal();
    }

    /// Vykoná príkaz `id` s parametrami `params` (JSON objekt alebo `null`).
    ///
    /// Meniaci príkaz uloží krok späť, po úspechu sa model prepočíta. Pri chybe alebo
    /// panike príkazu ostane dokument v stave pred príkazom.
    ///
    /// Parameter `coalesce: true` zlúči príkaz s predchádzajúcim rovnakým krokom
    /// (napr. pri ťahaní posuvníka) do jedného kroku späť.
    pub fn run(&mut self, id: &str, params: Value) -> Result<Value, EngineError> {
        let spec = cmd::find(id).ok_or_else(|| EngineError::UnknownCommand(id.to_owned()))?;
        if let Err(reason) = (spec.enabled)(self) {
            return Err(EngineError::Disabled {
                command: id.to_owned(),
                reason,
            });
        }
        let (params, coalesce) = split_coalesce(params);
        self.flags_set = false;
        let before = match spec.effect {
            Effect::Edit => self.doc.as_ref().map(State::capture),
            _ => None,
        };
        let selection = self.selection.clone();

        let outcome = catch_unwind(AssertUnwindSafe(|| (spec.run)(self, params.clone())));
        let result = match outcome {
            Ok(r) => r,
            Err(panic) => Err(EngineError::Panicked {
                command: id.to_owned(),
                message: panic_message(&panic),
            }),
        };

        match result {
            Ok(value) => {
                if let (Effect::Edit, Some(before)) = (spec.effect, before) {
                    if let Err(e) = self.finish_edit(id, &params, coalesce, before) {
                        self.selection = selection;
                        return Err(e);
                    }
                } else if spec.effect != Effect::Read {
                    self.rebuild();
                }
                if spec.effect != Effect::Read || self.selection != selection {
                    self.version += 1;
                }
                Ok(value)
            }
            Err(e) => {
                if let Some(before) = before {
                    if let Some(doc) = self.doc.as_mut() {
                        before.restore(doc);
                    }
                }
                self.selection = selection;
                if matches!(e, EngineError::Panicked { .. }) {
                    self.rebuild();
                    self.version += 1;
                }
                Err(e)
            }
        }
    }

    /// Po meniacom príkaze: kontrola vstupu, zápis kroku späť, prepočet modelu.
    fn finish_edit(
        &mut self,
        id: &str,
        params: &Value,
        coalesce: bool,
        before: State,
    ) -> Result<(), EngineError> {
        let Some(doc) = self.doc.as_mut() else {
            return Ok(());
        };
        if before.same_content(doc) {
            return Ok(());
        }
        let mut problems = Vec::new();
        if doc.project.cabinets.len() > MAX_CABINETS {
            problems.push(tr!("engine.session.too_many_cabinets", max = MAX_CABINETS));
        }
        problems.extend(cc_config::validate_project(&doc.project, &doc.materials));
        if doc.standard != before.standard {
            problems.extend(cc_config::validate_standard(&doc.standard));
        }
        if !problems.is_empty() {
            before.restore(doc);
            return Err(EngineError::Failed(tr!(
                "engine.session.change_rejected",
                problems = problems.join("\n  - ")
            )));
        }
        // Príkaz, ktorý si príznaky neuložených zmien nastavil sám (napr. načítanie
        // štandardu zo súboru), ich nechá tak.
        if !std::mem::take(&mut self.flags_set) {
            if doc.project != before.project {
                doc.dirty = true;
            }
            if doc.standard != before.standard {
                doc.standard_dirty = true;
            }
        }
        let key = coalesce.then(|| format!("{id}:{}", merge_key(params)));
        self.history.push(id, key, before);
        self.rebuild();
        Ok(())
    }

    /// Nahradí dokument (otvorenie, nový projekt). Vymaže históriu a výber.
    pub(crate) fn set_document(&mut self, doc: Document) {
        self.doc = Some(doc);
        self.history.clear();
        self.selection = Selection::default();
        self.rebuild();
        self.version += 1;
    }

    pub(crate) fn doc_mut(&mut self) -> Result<&mut Document, EngineError> {
        self.doc
            .as_mut()
            .ok_or_else(|| EngineError::Failed(tr!("engine.errors.no_project")))
    }

    pub(crate) fn require_doc(&self) -> Result<&Document, EngineError> {
        self.doc
            .as_ref()
            .ok_or_else(|| EngineError::Failed(tr!("engine.errors.no_project")))
    }

    /// Prepočíta model z aktuálneho dokumentu.
    pub(crate) fn rebuild(&mut self) {
        self.built = self.doc.as_ref().map(|doc| {
            let r = catch_unwind(AssertUnwindSafe(|| {
                cc_pipeline::build(
                    &doc.project,
                    &doc.standard,
                    &doc.materials,
                    doc.machine.as_ref(),
                )
            }));
            match r {
                Ok(Ok(built)) => Ok(Arc::new(built)),
                Ok(Err(e)) => Err(e.to_string()),
                Err(panic) => Err(tr!(
                    "engine.session.model_panic",
                    message = panic_message(&panic)
                )),
            }
        });
    }

    pub(crate) fn undo_step(&mut self, redo: bool) -> Result<Value, EngineError> {
        let doc = self.doc_mut()?;
        let current = State::capture(doc);
        let step = if redo {
            self.history.redo(current)
        } else {
            self.history.undo(current)
        };
        let Some((label, state)) = step else {
            return Err(EngineError::Failed(if redo {
                tr!("engine.session.nothing_to_redo")
            } else {
                tr!("engine.session.nothing_to_undo")
            }));
        };
        if let Some(doc) = self.doc.as_mut() {
            state.restore(doc);
        }
        self.fix_selection();
        Ok(serde_json::json!({ "label": cmd::label_of(&label) }))
    }

    /// Zruší výber skrinky alebo dielu, ktoré už neexistujú.
    pub(crate) fn fix_selection(&mut self) {
        let Some(doc) = &self.doc else {
            self.selection = Selection::default();
            return;
        };
        if let Some(id) = &self.selection.cabinet {
            if !doc.project.cabinets.iter().any(|c| &c.id == id) {
                self.selection = Selection::default();
            }
        }
    }

    pub(crate) fn history_len(&self) -> (usize, usize) {
        (self.history.undo_len(), self.history.redo_len())
    }
}

/// Oddelí parameter `coalesce` od ostatných parametrov príkazu.
fn split_coalesce(params: Value) -> (Value, bool) {
    match params {
        Value::Object(mut map) => {
            let coalesce = map
                .remove("coalesce")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            (Value::Object(map), coalesce)
        }
        Value::Null => (Value::Object(Default::default()), false),
        other => (other, false),
    }
}

/// Kľúč zlučovania: ktorý objekt a ktoré polia príkaz mení (bez hodnôt).
fn merge_key(params: &Value) -> String {
    let Value::Object(map) = params else {
        return String::new();
    };
    let mut parts = Vec::new();
    for (k, v) in map {
        match v {
            Value::String(s) if k == "id" || k == "path" => parts.push(format!("{k}={s}")),
            Value::Object(inner) => {
                let mut keys: Vec<&str> = inner.keys().map(String::as_str).collect();
                keys.sort_unstable();
                parts.push(format!("{k}[{}]", keys.join(",")));
            }
            _ => parts.push(k.clone()),
        }
    }
    parts.sort();
    parts.join(";")
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        tr!("engine.session.unknown_panic")
    }
}
