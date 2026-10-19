//! Preklady textov aplikácie (ADR 0007).
//!
//! Texty sú v `locales/<kód>.toml` (jeden súbor na jazyk). Slovenčina a angličtina sú
//! vložené do programu; ďalšie jazyky (a opravy vstavaných) sa načítajú zo súborov
//! `locales/*.toml` vedľa programu alebo v `%APPDATA%\CabinetCRAFT\locales` bez
//! nového prekladu programu.
//!
//! ```ignore
//! use cc_i18n::tr;
//! ui.button(tr!("ribbon.return"));
//! ui.label(tr!("parts.count", count = n));
//! ```
//!
//! Chýbajúci kľúč: zvolený jazyk → angličtina → slovenčina → samotný kľúč. Nikdy
//! nespadne a nikdy nevráti prázdny text.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::todo,
        clippy::unimplemented,
        clippy::unreachable
    )
)]

mod catalog;
pub mod plural;

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

pub use catalog::{placeholders, Catalog, Message, ParseError};
pub use plural::Category;

/// Predvolený jazyk aplikácie.
pub const DEFAULT_LANGUAGE: &str = "sk";
/// Referenčný jazyk: prekladatelia kopírujú `en.toml`, chýbajúci text ide najprv z neho.
pub const REFERENCE_LANGUAGE: &str = "en";

/// Vstavané súbory (kód, obsah).
pub const BUILTIN: &[(&str, &str)] = &[
    ("sk", include_str!("../../../locales/sk.toml")),
    ("en", include_str!("../../../locales/en.toml")),
];

/// Hodnota parametra textu.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg<'a> {
    Str(Cow<'a, str>),
    Int(i64),
    Float(f64),
}

impl Arg<'_> {
    fn number(&self) -> Option<f64> {
        match self {
            Arg::Int(i) => Some(*i as f64),
            Arg::Float(f) => Some(*f),
            Arg::Str(s) => s.trim().replace(',', ".").parse().ok(),
        }
    }
}

impl std::fmt::Display for Arg<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Arg::Str(s) => f.write_str(s),
            Arg::Int(i) => write!(f, "{i}"),
            Arg::Float(v) => write!(f, "{v}"),
        }
    }
}

impl<'a> From<&'a str> for Arg<'a> {
    fn from(v: &'a str) -> Self {
        Arg::Str(Cow::Borrowed(v))
    }
}
impl<'a> From<&'a String> for Arg<'a> {
    fn from(v: &'a String) -> Self {
        Arg::Str(Cow::Borrowed(v))
    }
}
impl From<String> for Arg<'_> {
    fn from(v: String) -> Self {
        Arg::Str(Cow::Owned(v))
    }
}
impl<'a> From<Cow<'a, str>> for Arg<'a> {
    fn from(v: Cow<'a, str>) -> Self {
        Arg::Str(v)
    }
}
macro_rules! arg_int {
    ($($t:ty),*) => {$(
        impl From<$t> for Arg<'_> {
            fn from(v: $t) -> Self {
                Arg::Int(i64::try_from(v).unwrap_or(i64::MAX))
            }
        }
        impl From<&$t> for Arg<'_> {
            fn from(v: &$t) -> Self {
                Arg::Int(i64::try_from(*v).unwrap_or(i64::MAX))
            }
        }
    )*};
}
arg_int!(i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);
impl From<f64> for Arg<'_> {
    fn from(v: f64) -> Self {
        Arg::Float(v)
    }
}
impl From<f32> for Arg<'_> {
    fn from(v: f32) -> Self {
        Arg::Float(f64::from(v))
    }
}

/// Text pre kľúč v zvolenom jazyku.
///
/// `tr!("kľúč")`, `tr!("kľúč", meno = hodnota, count = n)`. Tvar množného čísla vyberie
/// parameter `count` (inak prvý číselný parameter).
#[macro_export]
macro_rules! tr {
    ($key:expr $(,)?) => {
        $crate::tr($key)
    };
    ($key:expr, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::tr_args($key, &[$((stringify!($name), $crate::Arg::from($value))),+])
    };
}

/// Text pre kľúč v zvolenom jazyku (bez parametrov).
pub fn tr(key: &str) -> String {
    current().text(key, &[])
}

/// Text pre kľúč s parametrami `{meno}`.
pub fn tr_args(key: &str, args: &[(&str, Arg<'_>)]) -> String {
    current().text(key, args)
}

/// Kód zvoleného jazyka.
pub fn language() -> String {
    current().primary.code.clone()
}

/// Je kľúč v referenčnom alebo predvolenom jazyku? (Pre kód, ktorý skladá kľúč za behu.)
pub fn has_key(key: &str) -> bool {
    let b = builtins();
    b.en.contains(key) || b.sk.contains(key)
}

/// Zvolený jazyk a záložné jazyky.
#[derive(Debug, Clone)]
pub struct Translator {
    primary: Arc<Catalog>,
    fallbacks: Vec<Arc<Catalog>>,
}

impl Translator {
    /// Prekladač pre katalóg so zálohou na vstavanú angličtinu a slovenčinu.
    pub fn new(primary: Catalog) -> Translator {
        let b = builtins();
        Translator {
            primary: Arc::new(primary),
            fallbacks: vec![b.en.clone(), b.sk.clone()],
        }
    }

    pub fn code(&self) -> &str {
        &self.primary.code
    }

    /// Text pre kľúč; nikdy nie je prázdny (posledná záloha je kľúč).
    pub fn text(&self, key: &str, args: &[(&str, Arg<'_>)]) -> String {
        let found = std::iter::once(&self.primary)
            .chain(self.fallbacks.iter())
            .find_map(|c| c.get(key).map(|m| (m, c.plural.as_str())));
        let Some((message, rule)) = found else {
            return if key.is_empty() {
                "?".to_owned()
            } else {
                key.to_owned()
            };
        };
        let count = args
            .iter()
            .find(|(n, _)| *n == "count")
            .or_else(|| {
                args.iter()
                    .find(|(_, a)| matches!(a, Arg::Int(_) | Arg::Float(_)))
            })
            .and_then(|(_, a)| a.number());
        let category = count.map(|n| plural::category(rule, n));
        let out = format(message.text_for(category), args);
        if out.trim().is_empty() {
            key.to_owned()
        } else {
            out
        }
    }
}

/// Dosadí parametre `{meno}`; `{{` a `}}` sú zátvorky. Neznámy parameter ostane v texte.
pub fn format(template: &str, args: &[(&str, Arg<'_>)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(pos) = rest.find(['{', '}']) {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
            continue;
        }
        if tail.starts_with('{') {
            if let Some(end) = tail.find('}') {
                let name = &tail[1..end];
                if catalog::is_name(name) {
                    match args.iter().find(|(n, _)| *n == name) {
                        Some((_, value)) => out.push_str(&value.to_string()),
                        None => out.push_str(&tail[..=end]),
                    }
                    rest = &tail[end + 1..];
                    continue;
                }
            }
        }
        out.push_str(&tail[..1]);
        rest = &tail[1..];
    }
    out.push_str(rest);
    out
}

// ----- vstavané jazyky a globálny stav -----

struct Builtins {
    en: Arc<Catalog>,
    sk: Arc<Catalog>,
}

fn builtin_catalog(code: &str) -> Catalog {
    BUILTIN
        .iter()
        .find(|(c, _)| *c == code)
        .and_then(|(c, text)| Catalog::parse(c, text).ok())
        .unwrap_or_else(|| Catalog::empty(code))
}

fn builtins() -> &'static Builtins {
    static B: OnceLock<Builtins> = OnceLock::new();
    B.get_or_init(|| Builtins {
        en: Arc::new(builtin_catalog("en")),
        sk: Arc::new(builtin_catalog("sk")),
    })
}

static CURRENT: RwLock<Option<Arc<Translator>>> = RwLock::new(None);
static DIRS: RwLock<Option<Vec<PathBuf>>> = RwLock::new(None);

fn current() -> Arc<Translator> {
    if let Some(t) = CURRENT.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return t.clone();
    }
    let t = Arc::new(Translator::new(builtin_catalog(DEFAULT_LANGUAGE)));
    let mut slot = CURRENT.write().unwrap_or_else(|e| e.into_inner());
    slot.get_or_insert(t).clone()
}

/// Chyba prepnutia jazyka.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum I18nError {
    /// Jazyk nie je vstavaný a nie je preň súbor v priečinkoch `locales`.
    Unknown(String),
    /// Súbor prekladu sa nedá prečítať.
    File { path: PathBuf, message: String },
}

impl std::fmt::Display for I18nError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            I18nError::Unknown(code) => write!(f, "language `{code}` not found"),
            I18nError::File { path, message } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for I18nError {}

/// Priečinky s ďalšími jazykmi: `locales` vedľa programu a `%APPDATA%\CabinetCRAFT\locales`.
pub fn default_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        dirs.push(exe_dir.join("locales"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("CabinetCRAFT").join("locales"));
    }
    dirs
}

/// Nastaví priečinky s jazykmi (testy, CLI); `None` = [`default_dirs`].
pub fn set_search_dirs(dirs: Option<Vec<PathBuf>>) {
    *DIRS.write().unwrap_or_else(|e| e.into_inner()) = dirs;
}

/// Aktuálne priečinky s jazykmi (neskorší má prednosť).
pub fn search_dirs() -> Vec<PathBuf> {
    DIRS.read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_else(default_dirs)
}

/// Kód jazyka z mena súboru (`de`, `pt-BR`); iné mená sa ignorujú.
fn valid_code(code: &str) -> bool {
    (2..=16).contains(&code.len())
        && code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && code.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

/// Súbory `<kód>.toml` v priečinkoch jazykov (v poradí priečinkov).
fn language_files(dirs: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let mut files = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut here: Vec<(String, PathBuf)> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("toml"))
            })
            .filter_map(|p| {
                let code = p.file_stem()?.to_str()?.to_owned();
                valid_code(&code).then_some((code, p))
            })
            .collect();
        here.sort();
        files.extend(here);
    }
    files
}

/// Jazyk v prepínači.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    pub code: String,
    pub name: String,
    /// Vstavaný v programe (aj keď ho súbor v `locales` opravuje).
    pub builtin: bool,
    /// Súbory zo priečinkov `locales`, ktoré jazyk tvoria alebo opravujú.
    pub files: Vec<PathBuf>,
    /// Chyba čítania súboru (jazyk sa nedá zvoliť, ak nie je vstavaný).
    pub error: Option<String>,
}

/// Dostupné jazyky: vstavané (sk, en) a súbory v priečinkoch `locales`.
pub fn languages() -> Vec<Language> {
    let mut out: Vec<Language> = BUILTIN
        .iter()
        .map(|(code, _)| {
            let c = builtin_catalog(code);
            Language {
                code: (*code).to_owned(),
                name: c.name,
                builtin: true,
                files: Vec::new(),
                error: None,
            }
        })
        .collect();
    for (code, path) in language_files(&search_dirs()) {
        let parsed = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| Catalog::parse(&code, &t).map_err(|e| e.message));
        let idx = match out.iter().position(|l| l.code.eq_ignore_ascii_case(&code)) {
            Some(i) => i,
            None => {
                out.push(Language {
                    code: code.clone(),
                    name: code.clone(),
                    builtin: false,
                    files: Vec::new(),
                    error: None,
                });
                out.len() - 1
            }
        };
        let Some(lang) = out.get_mut(idx) else {
            continue;
        };
        lang.files.push(path);
        match parsed {
            Ok(c) if c.name != c.code => lang.name = c.name,
            Ok(_) => {}
            Err(e) => lang.error = Some(e),
        }
    }
    let builtin_count = BUILTIN.len().min(out.len());
    if let Some(rest) = out.get_mut(builtin_count..) {
        rest.sort_by_key(|l| l.name.to_lowercase());
    }
    out
}

/// Katalóg jazyka: vstavaný a nad ním súbory z priečinkov `locales`.
pub fn load(code: &str) -> Result<Catalog, I18nError> {
    let builtin = BUILTIN.iter().find(|(c, _)| c.eq_ignore_ascii_case(code));
    let mut catalog = match builtin {
        Some((c, _)) => builtin_catalog(c),
        None => Catalog::empty(code),
    };
    let mut found = builtin.is_some();
    for (file_code, path) in language_files(&search_dirs()) {
        if !file_code.eq_ignore_ascii_case(code) {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| I18nError::File {
            path: path.clone(),
            message: e.to_string(),
        })?;
        let parsed = Catalog::parse(&catalog.code, &text).map_err(|e| I18nError::File {
            path: path.clone(),
            message: e.message,
        })?;
        catalog.overlay(parsed);
        found = true;
    }
    if found {
        Ok(catalog)
    } else {
        Err(I18nError::Unknown(code.to_owned()))
    }
}

/// Prepne jazyk. Pri chybe ostane predchádzajúci jazyk.
pub fn set_language(code: &str) -> Result<(), I18nError> {
    let catalog = load(code.trim())?;
    let t = Arc::new(Translator::new(catalog));
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = Some(t);
    Ok(())
}

// ----- kontrola kľúčov (cargo xtask i18n, testy) -----

/// Rozdiel kľúčov jazyka voči referenčnému.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyDiff {
    /// V referencii sú, v jazyku chýbajú.
    pub missing: Vec<String>,
    /// V jazyku sú navyše (preklep alebo zrušený text).
    pub extra: Vec<String>,
    /// Text používa iné parametre `{…}` než referencia.
    pub placeholders: Vec<String>,
}

impl KeyDiff {
    pub fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.extra.is_empty() && self.placeholders.is_empty()
    }
}

fn message_placeholders(m: &Message) -> Vec<String> {
    let mut all: Vec<String> = m.texts().into_iter().flat_map(placeholders).collect();
    all.sort();
    all.dedup();
    all
}

/// Porovná kľúče a parametre jazyka s referenciou.
pub fn diff(reference: &Catalog, other: &Catalog) -> KeyDiff {
    let mut d = KeyDiff::default();
    for key in reference.keys() {
        match (reference.get(key), other.get(key)) {
            (Some(r), Some(o)) => {
                let (pr, po) = (message_placeholders(r), message_placeholders(o));
                // plurál môže v tvare „one“ vynechať {count}
                if po.iter().any(|p| !pr.contains(p))
                    || (!matches!(o, Message::Plural(_)) && pr != po)
                {
                    d.placeholders.push(format!(
                        "{key}: {{{}}} ≠ {{{}}}",
                        pr.join("}, {"),
                        po.join("}, {")
                    ));
                }
            }
            _ => d.missing.push(key.to_owned()),
        }
    }
    for key in other.keys() {
        if !reference.contains(key) {
            d.extra.push(key.to_owned());
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(code: &str, text: &str) -> Translator {
        Translator::new(Catalog::parse(code, text).unwrap())
    }

    #[test]
    fn builtin_files_parse_and_match() {
        for (code, text) in BUILTIN {
            let c = Catalog::parse(code, text).unwrap();
            assert!(c.warnings.is_empty(), "{code}: {:?}", c.warnings);
            assert_ne!(c.name, *code, "{code}: chýba [meta] name");
        }
        let d = diff(&builtin_catalog("en"), &builtin_catalog("sk"));
        assert!(d.is_empty(), "sk.toml voči en.toml: {d:#?}");
    }

    #[test]
    fn formats_parameters() {
        let a = [("n", Arg::from(3)), ("name", Arg::from("Stena W1"))];
        assert_eq!(format("{name}: {n} ks", &a), "Stena W1: 3 ks");
        assert_eq!(format("{{n}} {chyba} {", &a), "{n} {chyba} {");
        assert_eq!(format("}{ x}", &a), "}{ x}");
        assert_eq!(format("ä{n}č", &a), "ä3č");
    }

    #[test]
    fn plural_by_count() {
        let tr = t(
            "sk",
            "[a]\nparts = { one = \"{count} diel\", few = \"{count} diely\", other = \"{count} dielov\" }",
        );
        let p = |n: usize| tr.text("a.parts", &[("count", Arg::from(n))]);
        assert_eq!(p(1), "1 diel");
        assert_eq!(p(3), "3 diely");
        assert_eq!(p(7), "7 dielov");
        assert_eq!(tr.text("a.parts", &[]), "{count} dielov");
    }

    #[test]
    fn fallback_never_empty() {
        let tr = t("xx", "[meta]\nname = \"X\"\n[a]\nb = \"B\"");
        assert_eq!(tr.text("a.b", &[]), "B");
        assert_eq!(tr.text("chyba.kluc", &[]), "chyba.kluc");
        assert_eq!(tr.text("", &[]), "?");
        // bez súboru: vstavaná angličtina má meno jazyka
        assert_eq!(tr.text("meta.name", &[]), "meta.name");
    }

    #[test]
    fn diff_reports_keys_and_placeholders() {
        let r = Catalog::parse("en", "[a]\nx = \"{n} x\"\ny = \"Y\"").unwrap();
        let o = Catalog::parse("de", "[a]\nx = \"{m} x\"\nz = \"Z\"").unwrap();
        let d = diff(&r, &o);
        assert_eq!(d.missing, ["a.y"]);
        assert_eq!(d.extra, ["a.z"]);
        assert_eq!(d.placeholders.len(), 1);
    }

    #[test]
    fn languages_from_folder_without_rebuild() {
        let dir = std::env::temp_dir().join(format!("cc-i18n-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("de.toml"), "[meta]\nname = \"Deutsch\"\n").unwrap();
        std::fs::write(dir.join("zz.toml"), "a = \"neukončený").unwrap();
        std::fs::write(dir.join("readme.txt"), "x").unwrap();
        set_search_dirs(Some(vec![dir.clone()]));
        let langs = languages();
        let codes: Vec<&str> = langs.iter().map(|l| l.code.as_str()).collect();
        assert_eq!(codes, ["sk", "en", "de", "zz"]);
        assert_eq!(langs[2].name, "Deutsch");
        assert!(langs[3].error.is_some());
        let de = load("de").unwrap();
        assert_eq!(de.name, "Deutsch");
        assert!(matches!(load("zz"), Err(I18nError::File { .. })));
        assert!(matches!(load("qq"), Err(I18nError::Unknown(_))));
        set_search_dirs(None);
        std::fs::remove_dir_all(&dir).ok();
    }
}
