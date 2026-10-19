//! Katalógy skriniek dielne: zoznam, nový katalóg, uloženie skrinky zo zákazky, odstránenie.
//!
//! Katalógy sú súbory `catalogs/<meno>.toml` vedľa projektu a zapisujú sa hneď (ako knižnica
//! v CV); nie sú súčasťou zákazky, preto nemajú krok späť.

use cc_i18n::tr;
use cc_model::{valid_catalog_name, Catalog, CatalogItem, NamedCatalog};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{has_doc, parse, CommandSpec, Effect};
use crate::{EngineError, Session};

/// Najviac položiek v jednom katalógu (ako kontrola pri načítaní).
const MAX_ITEMS: usize = 1000;

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "catalog.list",
        menu: false,
        shortcut: None,
        params: r#"{} – katalógy [{name, items}]; {"catalog": "Kuchyne_CASTEL"} – položky katalógu"#,
        effect: Effect::Read,
        enabled: has_doc,
        run: list,
    },
    CommandSpec {
        id: "catalog.create",
        menu: true,
        shortcut: None,
        params: r#"{"name": "Moje skrinky"} – súbor catalogs/<name>.toml"#,
        effect: Effect::Other,
        enabled: has_doc,
        run: create,
    },
    CommandSpec {
        id: "catalog.save_item",
        menu: true,
        shortcut: None,
        params: r#"{"catalog": "Moje skrinky", "cabinet": "SP1", "id"?: "S_MOJA", "name"?: "Spodná – 2 zásuvky", "group"?: "Spodné"} – rovnaké id prepíše položku"#,
        effect: Effect::Other,
        enabled: has_doc,
        run: save_item,
    },
    CommandSpec {
        id: "catalog.remove_item",
        menu: false,
        shortcut: None,
        params: r#"{"catalog": "Moje skrinky", "id": "S_MOJA"}"#,
        effect: Effect::Other,
        enabled: has_doc,
        run: remove_item,
    },
];

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct List {
    catalog: Option<String>,
}

fn list(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: List = if params.is_null() {
        List::default()
    } else {
        parse("catalog.list", params)?
    };
    let doc = s.require_doc()?;
    match p.catalog {
        None => Ok(json!(doc
            .catalogs
            .iter()
            .map(|c| json!({ "name": c.name, "items": c.catalog.items.len() }))
            .collect::<Vec<_>>())),
        Some(name) => {
            let c = find(&doc.catalogs, &name)?;
            serde_json::to_value(&c.catalog.items).map_err(|e| EngineError::Failed(e.to_string()))
        }
    }
}

fn find<'a>(catalogs: &'a [NamedCatalog], name: &str) -> Result<&'a NamedCatalog, EngineError> {
    catalogs
        .iter()
        .find(|c| c.name == name)
        .ok_or_else(|| tr!("engine.document.catalog_missing", name = name).into())
}

fn bad(command: &str, message: impl Into<String>) -> EngineError {
    EngineError::Params {
        command: command.to_owned(),
        message: message.into(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    name: String,
}

fn create(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "catalog.create";
    let p: Create = parse(C, params)?;
    let name = p.name.trim().to_owned();
    if !valid_catalog_name(&name) {
        return Err(bad(C, tr!("engine.catalog.bad_name")));
    }
    let doc = s.doc_mut()?;
    if doc
        .catalogs
        .iter()
        .any(|c| c.name.to_lowercase() == name.to_lowercase())
        || doc.catalog_path(&name).exists()
    {
        return Err(tr!("engine.catalog.exists", name = name).into());
    }
    doc.catalogs.push(NamedCatalog {
        name: name.clone(),
        catalog: Catalog::default(),
    });
    if let Err(e) = doc.write_catalog(&name) {
        doc.catalogs.retain(|c| c.name != name);
        return Err(e.into());
    }
    Ok(json!({ "name": name, "path": doc.catalog_path(&name) }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveItem {
    catalog: String,
    cabinet: String,
    id: Option<String>,
    name: Option<String>,
    group: Option<String>,
}

fn save_item(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "catalog.save_item";
    let p: SaveItem = parse(C, params)?;
    let doc = s.doc_mut()?;
    let spec = doc
        .project
        .cabinets
        .iter()
        .find(|c| c.id == p.cabinet)
        .cloned()
        .ok_or_else(|| EngineError::from(tr!("engine.errors.cabinet_missing", id = &p.cabinet)))?;
    let id = p.id.unwrap_or_else(|| spec.id.clone()).trim().to_owned();
    if id.is_empty() || id.chars().count() > 100 {
        return Err(bad(C, tr!("engine.catalog.bad_item_id")));
    }
    let Some(index) = doc.catalogs.iter().position(|c| c.name == p.catalog) else {
        return Err(tr!("engine.document.catalog_missing", name = &p.catalog).into());
    };
    let before = doc.catalogs.get(index).map(|c| c.catalog.clone());
    let Some(cat) = doc.catalogs.get_mut(index) else {
        return Err(tr!("engine.document.catalog_missing", name = &p.catalog).into());
    };
    let old = cat.catalog.items.iter().position(|i| i.id == id);
    let previous = old.and_then(|i| cat.catalog.items.get(i));
    let name = p
        .name
        .or_else(|| previous.map(|i| i.name.clone()))
        .unwrap_or_else(|| id.clone());
    let group = p
        .group
        .or_else(|| previous.map(|i| i.group.clone()))
        .unwrap_or_default();
    if name.chars().count() > 200 || group.chars().count() > 100 {
        return Err(bad(C, tr!("engine.catalog.name_too_long")));
    }
    let item = CatalogItem::from_spec(&spec, &id, name.trim(), group.trim());
    match old.and_then(|i| cat.catalog.items.get_mut(i)) {
        Some(slot) => *slot = item,
        None => {
            if cat.catalog.items.len() >= MAX_ITEMS {
                return Err(tr!("engine.catalog.too_many_items", max = MAX_ITEMS).into());
            }
            cat.catalog.items.push(item);
        }
    }
    if let Err(e) = doc.write_catalog(&p.catalog) {
        // nezapísané: katalóg v pamäti vrátime
        if let (Some(cat), Some(before)) = (doc.catalogs.get_mut(index), before) {
            cat.catalog = before;
        }
        return Err(e.into());
    }
    Ok(json!({ "catalog": p.catalog, "id": id, "replaced": old.is_some() }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveItem {
    catalog: String,
    id: String,
}

fn remove_item(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "catalog.remove_item";
    let p: RemoveItem = parse(C, params)?;
    let doc = s.doc_mut()?;
    let Some(cat) = doc.catalogs.iter_mut().find(|c| c.name == p.catalog) else {
        return Err(tr!("engine.document.catalog_missing", name = &p.catalog).into());
    };
    let before = cat.catalog.clone();
    let n = cat.catalog.items.len();
    cat.catalog.items.retain(|i| i.id != p.id);
    if cat.catalog.items.len() == n {
        return Err(tr!("engine.catalog.item_missing", id = &p.id, name = &p.catalog).into());
    }
    if let Err(e) = doc.write_catalog(&p.catalog) {
        if let Some(cat) = doc.catalogs.iter_mut().find(|c| c.name == p.catalog) {
            cat.catalog = before;
        }
        return Err(e.into());
    }
    Ok(Value::Null)
}
