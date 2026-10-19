# Riadiaci kanál CabinetCRAFT

Agent, testy a neskôr MCP ovládajú CabinetCRAFT cez **JSON riadky na `127.0.0.1:PORT`**: jedna požiadavka na riadok, jedna odpoveď na riadok. Server je v crate `cc-control`.

Spustenie:
- bez okna: `cabinetcraft serve projekt.toml [--control PORT]`. Na začiatku vypíše `{"port": N}` (pri `--control 0` alebo bez neho si vyberie voľný port). Metódy `ui.*` tu nie sú k dispozícii.
- s oknom: `cabinetcraft-app --control PORT projekt.toml` (alebo premenná `CABINETCRAFT_CONTROL_PORT`). Okno zvládne všetky metódy jadra aj metódy `ui.*` nižšie.

```json
{"id": 1, "method": "engine.execute", "params": {"command": "cabinet.add", "params": {"type": "base", "w": 450}}}
{"id": 1, "ok": true, "result": {"id": "SP2"}}
```

Chyba: `{"id": …, "ok": false, "kind": "…", "error": "text"}`. Druhy (`kind`): `unknown-method`, `unknown-command`, `disabled`, `params`, `failed`, `panicked`, `unavailable`, `timeout`, `closed`.

**Čakanie:** server čaká na odpoveď aplikácie najviac 60 s. Potom odpovie `timeout` so stavom:
- `"state": "not-run"`: aplikácia požiadavku ešte nezačala. Je zrušená a nikdy sa nevykoná, takže poslať ju znova je bezpečné.
- `"state": "may-have-run"`: aplikácia ju už vybavovala. Pred opakovaním over stav (`project.inspect`).

## Metódy

Zdroj pravdy je `control.methods`. Stav k 2026-10-16:

| Metóda | Parametre | Čo robí |
|---|---|---|
| `ping` | | `"pong"`; odpovie aj vtedy, keď je aplikácia zaneprázdnená |
| `control.methods` | | zoznam metód tohto servera |
| `engine.commands` | | register príkazov: `id`, `label`, `menu`, `shortcut`, `params`, `effect`, `enabled`, `disabledReason` |
| `engine.execute` (alias `command`) | `{command, params}` | vykoná príkaz z registra; nikdy neotvorí dialóg |
| `project.inspect` | `{full?}` | skrinky, steny, neuložené zmeny, výber, história späť/znova |
| `model.inspect` | `{parts?, part?}` | počty, zistenia kontroly, zoznam dielov, jeden diel s operáciami |
| `app.open` | `{path}` | otvorí projekt |
| `app.save` | `{path?}` | uloží; s `path` ako Uložiť ako |
| `app.quit` | `{force?}` | ukončí `serve`; v okne ho zatvorí, pri neuložených zmenách iba s `force: true` |

### Metódy okna

| Metóda | Parametre | Čo robí |
|---|---|---|
| `ui.inspect` | | záložka, jazyk (`language`, kód zvoleného jazyka) a dostupné jazyky (`languages`: `code`, `name`, `builtin`, `error`), kamera, voľby 3D, výber, stavový riadok, dialóg, rozmer okna, fokus klávesnice; `renderer` (`wgpu: …` alebo `cpu`), `view3d` (poloha a veľkosť 3D výrezu), `frame` a `repaintCauses` (kto vyžiadal poslednú snímku) |
| `ui.set` | `{tab?, planMode?: home\|walls\|objects\|floors\|ceilings\|tops\|molding\|autofill\|section\|cad, cabinetView?: section\|face\|plan\|end\|3d\|reports, partView?: cam\|front\|top\|end\|3d\|reports, camera?: {yaw, pitch, zoom, pan, perspective}, resetCamera?, view?: front\|top\|iso\|selection\|default, scene?: {backs, shelves, operations, walls, explode}, csv?: {merge, decimal_comma}, language?: sk\|en\|kód}` | jazyk okna, výkresov a štítkov (`language`: vstavaný `sk`/`en` alebo kód súboru `locales/<kód>.toml` vedľa programu či v `%APPDATA%\CabinetCRAFT\locales`; prepne sa bez reštartu a zapamätá v `%APPDATA%\CabinetCRAFT\nastavenia.toml`, neznámy jazyk alebo poškodený súbor = chyba `params` a jazyk ostane), záložka (`plan`, `elevation`, `3d`, `parts`, `part`, `issues`, `standard`), režim ľavého panela aktuálnej záložky pôdorysu alebo pohľadu (`planMode`, ako F2–F7; v pohľade aj `splash`, `grain`), pohľad (`view`: spredu, zhora, izo, priblíženie na výber, pôvodný) a voľby; záložka editora skrinky (`cabinetView`, záložka `cabinet`) a otvorenej dosky (`partView`, záložka `part_edit`, doska = vybraný diel); `ui.inspect` vracia `plan_mode`, `elevation_mode`, `cabinet_view` a `part_view` |
| `ui.screenshot` | `{path}` | PNG celého okna; odpovie, keď je snímka zapísaná (okno nesmie byť minimalizované) |
| `ui.render` | `{path, width?, height?, window?}` | 3D náhľad do PNG softvérovým rastrom (aj pri zamknutej obrazovke), s aktuálnou kamerou, voľbami a výberom; `window: true` = rozmer a mierka 3D výrezu okna, obraz sa dá porovnať so snímkou okna |
| `ui.bench` | `{frames?: 240, yawStep?: 0.01}` | meranie plynulosti: prepne na 3D, otáča kamerou `frames` snímok, vráti `fps`, `frameMs` (avg, p50, p95, max), `updateCpuMs`, `renderer`, `parts`, `triangles`; kamera sa vráti |
| `ui.click` | `{x, y, button?, double?}` | klik v bodoch okna |
| `ui.drag` | `{x, y, to: [x, y], button?}` | stlačenie, posun a pustenie |
| `ui.key` | `{key, ctrl?, shift?, alt?}` | kláves (mená ako `egui::Key`: `Enter`, `Escape`, `Z`, `F2` …) |
| `ui.text` | `{text}` | text do poľa s fokusom |

Vstup (`ui.click` … `ui.text`) sa iba zaradí do frontu a spracuje sa v nasledujúcich snímkach okna. Odpoveď obsahuje `queuedFrames`. Výsledok over po krátkej pauze cez `ui.inspect`, `project.inspect` alebo snímku.

## Príkazy (`engine.execute`)

Úplný zoznam s parametrami: `engine.commands` alebo `cabinetcraft commands`.

- Projekt: `project.open`, `project.new`, `project.save`, `project.save_as`, `project.rename`, `project.info` (`{"number", "customer", "designer", "company", "logo"}`; `logo` je cesta k PNG/JPEG relatívna k projektu a musí ísť načítať; prázdny text alebo `null` pole zmaže), `defaults.set`.
- Kovanie: `model.hardware {"cabinet"?}` – súpis kovania (pánty, zásuvky, kolíky, konfirmáty, nožičky, podperky, čočky, TIP-ON).
- Katalógy (`catalogs/<meno>.toml` vedľa projektu, zapisujú sa hneď, bez kroku späť): `catalog.list` (`{}` katalógy, `{"catalog": meno}` položky), `catalog.create {"name"}`, `catalog.save_item {"catalog", "cabinet", "id"?, "name"?, "group"?}` (skrinka zo zákazky do katalógu, rovnaké id prepíše), `catalog.remove_item {"catalog", "id"}`; `cabinet.add {"catalog": "S_KUCH_D", "from"?: "Kuchyne_CASTEL", "w"?, …}` pridá skrinku z katalógu.
- Skrinky: `cabinet.add`, `cabinet.remove`, `cabinet.duplicate`, `cabinet.move`, `cabinet.set` (`{"id": "SP1", "set": {"w": 650}}`, polia ako `[[cabinet]]` v `project.toml`; čelá `"fronts": [{"kind": "drawer", "height": 180}, {"kind": "door", "hinge_side": "right"}]`, `kind` = `door`|`pair`|`drawer`|`false_front`|`open`, `"front_mount": "overlay"|"inset"`, `"hinge": "blum-110-inserta"`, `"drawer": "merivobox"`, ADR 0006).
- Štandard: `standard.set` (`{"path": "dowel.face_depth", "value": 12}`), `standard.use`, `standard.save`, `standard.reload`.
- Úpravy: `edit.undo`, `edit.redo`, `selection.set`.
- Výstup: `export.machine` (pri chybách kontroly sa nevykoná), `export.drawings` (výkresy do PDF, výsledok `{path, pages, bytes, warnings}`: `{"path"?, "sheets"?: ["job", "walls", "cabinets", "parts", "wall:ID", "cabinet:ID", "part:ID"]; predvolene zákazka, steny so skrinkami alebo otvormi, skrinky a diely, "parts"?: "all"|"machined"}`).
- Čítanie: `project.inspect`, `model.inspect`.

Meniaci príkaz (`effect: "edit"`) vytvorí krok späť. Neplatná zmena (záporný rozmer, neznáme pole, duplicitné `id`…) sa odmietne a projekt ostane bez zmeny. Parameter `"coalesce": true` zlúči opakované zmeny toho istého poľa do jedného kroku späť (ťahanie posuvníka).

## Skript bez servera

`cabinetcraft run projekt.toml --script skript.jsonl [--save]` vykoná riadky `{"command": "…", "params": {…}}` a každý výsledok vypíše ako riadok JSON.
