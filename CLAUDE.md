# CabinetCRAFT – pokyny pre agentov

CabinetCRAFT je vlastná aplikácia v čistom Ruste na **interaktívny návrh korpusového nábytku v 3D s výstupom na CNC**. Funkčne sa zameriava na to, čo dielňa robí v Hexagon CABINET VISION: miestnosť → skrinky → diely → spoje a kovanie → rozpis, štítky a programy pre stroj. Hlavný stroj je **SCM CX100 (Maestro `.xcs`)**. Beží na Windows ako jeden `.exe`.

Používateľ (vlastník projektu) píše po slovensky. Dokumentácia a komentáre sú po slovensky, identifikátory v kóde po anglicky. Texty, ktoré vidí používateľ (okno, stavový riadok, chyby príkazov, výkresy a štítky PDF), nie sú v kóde: `tr!("sekcia.kľúč")` z `cc-i18n` a text v `locales/sk.toml` aj `locales/en.toml` (ADR 0007, `cargo xtask i18n` v CI). Slovenčina je predvolená; programy pre stroj, CSV a `model.json` sa neprekladajú.

## Začni každú session tu
Priečinok `plan/` (stav, plán, ADR) je interný a vo verejnom repozitári na GitHube nie je; bez neho začni týmto súborom, `README.md` a `docs/`.

1. Prečítaj `plan/STATUS.md` (aktuálna fáza, ďalšia úloha), potom danú úlohu v `plan/execution-plan.md` a súvisiacu časť `plan/architecture.md`.
2. Rozhodnutia s dôvodmi sú v `plan/adr/`. Rozhodnutie, ktoré tam nie je a nepatrí vlastníkovi, zapíš ako nové ADR.
3. Postupuj podľa protokolu samostatnej práce (`plan/execution-plan.md` §6). Nezastavuj sa s otázkou, kým rozhodnutie nie je v zozname rozhodnutí vlastníka (§5).
4. Po dokončení úlohy aktualizuj `plan/STATUS.md` (čo je hotové, ďalšia úloha, otvorené otázky).

Funkčné zadanie MVP je v `../SPECIFIKACIA_MVP.md`, analýza CV (iba koncepty) v `../CABINET_VISION_analyza.md`.

## Nikdy nespadni
Aplikácia drží rozpracovanú zákazku; pád = stratená práca. **Má prednosť pred novými funkciami.**
- V produkčnom kóde žiadne `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`; žiadne `unsafe`. Každý produkčný crate má v koreni `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented, clippy::unreachable))]` a v `Cargo.toml` `[lints] workspace = true` (`unsafe_code = "forbid"`). V testoch je `unwrap` povolený.
- Chyby sú `Result<T, E>` cez chybový typ crate-u a `?`. Nedokončená funkcia vráti chybu alebo „zatiaľ nie je k dispozícii“, nikdy nespadne.
- Čísla zo vstupu sú nepriateľské (TOML projektu, parametre príkazov, riadiaci kanál): `get()` namiesto `[i]`, kontrola NaN/∞ a záporných rozmerov, obmedzené počty (polic, dielov, otvorov).
- Posledná poistka: vykonanie príkazu beží pod `catch_unwind`; ak predsa unikne panika, projekt sa vráti do stavu pred príkazom a ohlási sa chyba.
- Každá oprava pádu prichádza s regresným testom.

## Bezpečnosť stroja
Program pre CNC, ktorý je zlý, môže zlomiť nástroj alebo poškodiť stroj.
- Akákoľvek zmena výstupu `.xcs`/`.mpr` zmení golden súbory (`tests/golden/`). Rozdiel skontroluj riadok po riadku a v commite napíš, čo sa zmenilo a prečo.
- Hodnotu alebo makro nikdy neoznač ako overené (`verified = true`, `[U]`) bez potvrdenia vlastníka zo skúšky na stroji.
- Kontrola (cc-validate) blokuje export pri chybách; túto poistku nevypínaj ani neobchádzaj.

## Neporušiteľné
- **Clean-room voči CABINET VISION** (podrobne `plan/adr/0003-clean-room-cv.md`):
  - Povolené: koncepty z interných poznámok mimo repa (`../CABINET_VISION_analyza.md`, `../rea-snapshots/findings/` – vlastné popisy formátov súborov, výstupov a správania CV); pozorovať spustený CV zvonka na syntetických zákazkách so snímkami iba v `plan/cv/` (v gite ignorované). Postup analýzy určuje ADR 0003 (interné).
  - Zakázané: kopírovať kód, dáta štandardov a knižníc Hexagonu, ikony či texty pomocníka; názvy ich interných funkcií a schému databázy v identifikátoroch nášho kódu (mená tried a prúdov priamo zo súboru `.cvj` smie importér použiť ako konštanty formátu). Kód aplikácie píš iba podľa vlastných popisov vo `findings/`.
  - Živú databázu CV nečítaj bez súhlasu vlastníka. Do CV/CV25/CV26 nikdy nezapisuj.
  - Do repa necommituj súbory vytvorené CV, okrem anonymizovaných programov dielne v `tests/fixtures/scm-cx100/` a `tests/fixtures/scm-nesting/` (bez komentárov a údajov zákazníka; schválil vlastník 2026-10-16).
- **Všetko je príkaz** (od fázy F1): každá akcia používateľa je `CommandSpec` v `crates/cc-engine/src/cmd/*` (id napr. `cabinet.add`, popis, parametre, JSON `run`, `enabled`). Ľavý panel, ponuky, klávesové skratky, skripty CLI aj riadiaci kanál volajú tie isté príkazy. Volanie cez JSON nikdy neotvorí dialóg.
- **Vrstvy** (kontroluje `cargo xtask layers`, `plan/architecture.md` §2): L0 `cc-model`, `cc-i18n` → L1 `cc-config`, `cc-generate`, `cc-joints`, `cc-validate`, `cc-room`, `cc-nest` → L2 `cc-out-*`, `cc-scene` → L3 `cc-pipeline` → L4 `cc-engine` → L5 `cc-control` → aplikácie (`cc-app` = okno, `cc-cli`). Nič pod L5 nezávisí od egui/eframe/wgpu/rfd/winit.
- **UI je tenké:** panely čítajú stav z `cc-engine` a menia ho iba príkazmi (`session.run(id, params)`).
- **Všetko je dáta:** rozmery, pravidlá spojov, kovanie a parametre obrábania sú v TOML. Kód obsahuje iba algoritmy. Výstup je deterministický (rovnaký vstup = bajtovo rovnaký výstup).
- **Iba Rust**, Windows je hlavná platforma.
- **Kontroly pred každým commitom:** `cargo xtask ci` (fmt, clippy `-D warnings`, testy, vrstvy). Commituj po každom ucelenom kroku, ktorý prejde, a pushuj na `origin` (interný Gitea dielne). Na `main` môže pracovať iná session; vlastnú prácu rob vo worktree na vlastnej vetve a `main` doň zmerguj pred každou väčšou zmenou.

## Spúšťanie a pozeranie aplikácie
- Okno: `cargo run --release -p cc-app -- examples/dielna/project.toml`.
- CLI: `cargo run -p cc-cli -- build examples/dielna/project.toml --out out`.
- **Riadiaci kanál:** `cabinetcraft-app.exe --control PORT projekt.toml` (okno) alebo `cabinetcraft serve projekt.toml --control PORT` (bez okna). JSON riadky na `127.0.0.1:PORT`:
  - `{"id":1,"method":"engine.execute","params":{"command":"cabinet.add","params":{"type":"base","w":600}}}`
  - `{"id":2,"method":"project.inspect","params":{}}`, `model.inspect`, `ui.inspect`
  - `{"id":3,"method":"ui.screenshot","params":{"path":"C:/…/shot.png"}}` a potom si PNG prečítaj; `ui.render` vykreslí 3D aj pri zamknutej obrazovke.
  - `ui.set` (záložka, kamera), `ui.click`/`ui.drag`/`ui.key`/`ui.text` (skutočný vstup okna, spracuje sa v ďalších snímkach), `app.quit` (`force: true` pri neuložených zmenách).
  - Zoznam metód: `control.methods`; popis protokolu `docs/control-protocol.md`. Okno pri otvorení zapíše projekt do `%APPDATA%\CabinetCRAFT\posledny_projekt.txt`; po skúškach na dočasnej kópii vráť pôvodný obsah.
- Skript bez okna: `cabinetcraft run projekt.toml --script skript.jsonl [--save]`; zoznam príkazov `cabinetcraft commands`.
- Snímka pri štarte (staršia cesta): `CABINETCRAFT_SCREENSHOT=out.png CABINETCRAFT_TAB=3d|plan|elevation|parts|part|issues|standard CABINETCRAFT_PART=ID`, `CABINETCRAFT_RIBBON=tools` (karta Nástroje); jazyk podľa `%APPDATA%\CabinetCRAFT\nastavenia.toml` (`language = "en"`), na skúšku s `APPDATA` nastaveným na dočasný priečinok.
- **Pri práci na UI sa na výsledok vždy pozri** (snímka, prečítaj PNG) a porovnaj so zámerom; snímky ukladaj do scratchpadu, nie do repa.
- Viac agentov naraz: každý má vlastný `CARGO_TARGET_DIR`, upravuje iba crate-y svojej úlohy a po skončení svoj target zmaže (miesto na disku).

## Prostredie (Windows)
- Súbory upravuj nástrojmi Write/Edit alebo skriptom v súbore. Inline Python/heredoc v Bash nástroji kazí `\r\n` a spätné apostrofy.
- Golden súbory `.xcs`, `.mpr`, `RUN.bat` majú CRLF a v gite sú binárne (`.gitattributes`).
- Maestro na vývojovom PC nemá licenciu: XConverter tu skončí s kódom 1. Overenie `.pgmx` a skúška na stroji sú úlohou vlastníka (O1).

## Konvencie modelu
- Jednotky mm (`f64`), pravotočivé sústavy; uhly v stupňoch iba na vstupe/výstupe.
- Plocha **F** dielu je vždy k interiéru, predná (olepená) hrana dielu je vždy **`Y0`**.
- Hodnoty štandardu nesú pôvod: `[P]` z programov CV dielne, `[U]` potvrdil vlastník, `[?]` odhad na overenie.
