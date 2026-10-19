<div align="center">

<img src="docs/images/logo.svg" alt="CabinetCRAFT" width="460">

**Navrhni korpusový nábytok v 3D a pošli ho rovno na CNC.**<br>
Miestnosť → skrinky → diely → spoje a kovanie → rozpis, štítky a programy pre stroj.

[![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)](https://www.rust-lang.org/)
[![Platforma](https://img.shields.io/badge/platforma-Windows-0078D6?logo=windows)](#rýchly-štart)
[![Licencia](https://img.shields.io/badge/licencia-MIT%20OR%20Apache--2.0-blue)](#licencia)
[![Jazyky](https://img.shields.io/badge/UI-Sloven%C4%8Dina%20%7C%20English%20%7C%20tvoj-green)](locales/README.md)
[![Stav](https://img.shields.io/badge/stav-v%C3%BDvoj-yellow)](#čo-už-funguje)

[Snímky](#snímky) · [Prečo](#prečo-cabinetcraft) · [Čo funguje](#čo-už-funguje) · [Rýchly štart](#rýchly-štart) · [Stroje](#výstupy-pre-stroj) · [Automatizácia](#ovládanie-skriptom-a-agentmi) · [Architektúra](#architektúra) · [Preklady](#preklady) · [Plán](#plán)

🇬🇧 [English version](README.md) · 📖 [Podrobný technický popis](docs/podrobnosti.sk.md)

</div>

---

## Snímky

<table>
  <tr>
    <td width="50%"><img src="docs/images/ui-3d.png" alt="3D pohľad na kuchyňu"><br><sub><i>3D vzorovej kuchyne (<code>examples/dielna/kuchyna.toml</code>): steny, podlaha, skrinky s čelami.</i></sub></td>
    <td width="50%"><img src="docs/images/ui-plan.png" alt="Pôdorys"><br><sub><i>Pôdorys so zoznamom režimov (Steny F2, Objekty F3, … CAD F7), parametre miestnosti a kóty.</i></sub></td>
  </tr>
  <tr>
    <td><img src="docs/images/ui-elevation.png" alt="Pohľad na stenu"><br><sub><i>Pohľad na stenu: ťahaním posunieš skrinku, hranou zmeníš šírku, pravým klikom rozdelíš čelo.</i></sub></td>
    <td><img src="docs/images/ui-cabinet.png" alt="Editor skrinky"><br><sub><i>Skrinka otvorená samostatne: Rez, Čelo, Pôdorys, Bok, 3D a Zostavy – vidno každú dieru a kolík.</i></sub></td>
  </tr>
  <tr>
    <td colspan="2"><img src="docs/images/ui-board.png" alt="Editor dosky"><br><sub><i>Doska otvorená samostatne: pracovné roviny CAM, plocha s reťazami kót polôh dier, hrany, 3D a zoznam operácií, ktoré idú do stroja.</i></sub></td>
  </tr>
</table>

<sub>Snímky sú v anglickom rozhraní; prepína sa v Nástroje → Jazyk.</sub>

## Prečo CabinetCRAFT

Malá stolárska dielňa potrebuje od softvéru jedno: **diely z píly a z CNC musia sedieť**. Na tom je CabinetCRAFT postavený.

- **Jeden `.exe`, nič sa neinštaluje.** Čistý Rust, beží na bežnom PC s Windows a otvorí sa za sekundu.
- **Konštrukčný štandard dielne sú dáta, nie kód.** Hrúbky, drážka zadnej steny, rozloženie kolíkov, rady dier, pánty a výsuvy sú v čitateľnom súbore TOML. Každá hodnota má pôvod: `[P]` z reálnych programov, `[U]` potvrdila dielňa, `[?]` odhad na overenie.
- **Postup, na ktorý sú stolári zvyknutí.** Rozloženie zodpovedá programu, s ktorým veľa dielní vyrástlo: pôdorys s režimami, pohľad na stenu, samostatne otvorená skrinka aj doska, pás s *Návratom* a *Zoomom*. Ikony a texty sú vlastné.
- **Bezpečnosť stroja na prvom mieste.** Kontrola zablokuje export, keď sa diel nezmestí do stroja, diera je hlbšia ako doska alebo chýba nástroj. Šablóny neoverené na skutočnom stroji ostávajú `verified = false`.
- **Práca sa nestratí.** V produkčnom kóde žiadne `unwrap`/`panic`, každá zmena je príkaz so Späť, zlyhaný príkaz sa vráti.
- **Preložiť to vie ktokoľvek.** Všetky texty sú v [`locales/`](locales/README.md); nový jazyk je jeden súbor TOML, bez prekladania programu.

## Čo už funguje

| Oblasť | Čo dostaneš | Stav |
|---|---|---|
| **Miestnosť** | steny (čiara alebo obdĺžnik), okná a dvere, parametre miestnosti, skrinky pri stenách, rohy, automatické vyplnenie voľného miesta | ✅ |
| **Skrinky** | spodné, horné, vysoké; čelá (dvierka, dvojité, zásuvky, slepé čelá, otvorené), delenie sekcií, police, sokel, blendy, niky, šikmé skrinky, slepé rohy, LED rampy | ✅ |
| **Katalóg** | katalógy dielne v `catalogs/*.toml`, položku potiahneš na stenu, skrinku uložíš späť do katalógu | ✅ |
| **Spoje a kovanie** | kolíky, konfirmáty, drážka zadnej steny, rady dier pre police, pánty Blum s podložkami, zásuvky Merivobox, TIP-ON | ✅ |
| **Pohľady** | pôdorys, pohľad na stenu, 3D na GPU (DirectX 12, záloha na CPU), editor skrinky, editor dosky s rovinami CAM | ✅ |
| **Výstupy** | rozpis CSV, `model.json`, výkresy PDF (zákazka, steny, skrinky, diely), štítky 80 × 80 / 100 × 80 mm s čiarovým kódom Code 39 | ✅ |
| **SCM CX100** | Maestro `.xcs` pre každý diel + `RUN.bat` pre XConverter | ✅ formát podľa reálnych programov · ⚠️ najprv over na stroji |
| **SCM nesting** | rozloženie na tabule (MaxRects + žíhanie), programy tabúľ, delenie práce nesting / CX100, `CncRun.txt`, štítky v poradí rezania | ✅ · ⚠️ `verified = false` |
| **WoodWOP** | `.mpr` zo šablón (alternatívny výstup) | 🟡 `BohrHoriz`/`Nuten` neoverené |
| **Zákazky CABINET VISION** | `.cvj` sa otvorí priamo; Uložiť zapíše `MENO_cabinetcraft.cvj` vedľa pôvodnej (pôvodná sa nemení) | 🧪 skúšobné |
| **Jazyky** | slovenčina, angličtina; ďalšie cez `locales/*.toml` | ✅ |

> [!WARNING]
> Zlý program pre CNC môže zlomiť nástroj alebo poškodiť stroj. Pred prvým ostrým rezom spusti vygenerované programy na skúšobnom diele a skontroluj počiatok, smery osí a šírku drážky. Každá šablóna stroja, ktorú nikto nepotvrdil na skutočnom stroji, má `verified = false`.

## Rýchly štart

Potrebuješ [Rust](https://rustup.rs/) (stable) na Windows 10/11.

```sh
git clone https://github.com/LaMka21/CabinetCRAFT.git
cd CabinetCRAFT
cargo build --release -p cc-app
target\release\cabinetcraft-app.exe examples\dielna\kuchyna.toml
```

Pri prvom spustení sa otvorí vzorová kuchyňa, potom vždy posledný projekt. Program sa dá spustiť aj so zákazkou `.cvj`: `cabinetcraft-app.exe zakazka.cvj`.

| Kde | Ako |
|---|---|
| Pôdorys | **F2** steny, **F3** objekty, **F6** vyplnenie; *Návrat* späť na zoznam režimov |
| 3D | ľavé tlačidlo otáča · pravé posúva · koliesko zoom · klik = skrinka, ďalší klik = diel |
| Skrinka | dvojklik (v pôdoryse, pohľade alebo 3D) ju otvorí samostatne |
| Doska | dvojklik na dosku v editore skrinky |
| Export pre stroj | *Kontrola* → *Export pre stroj* (zablokovaný, kým kontrola hlási chyby) |
| Jazyk | *Nástroje* → *Jazyk* |

Príkazový riadok bez okna:

```sh
cargo run -p cc-cli -- build examples/dielna/project.toml --out out
```

zapíše `cutlist.csv`, `model.json`, `xcs/*.xcs` + `RUN.bat` a štítky. Chyby kontroly zastavia beh s kódom 1.

## Výstupy pre stroj

| Výstup | Odkiaľ | Poznámka |
|---|---|---|
| `xcs/<diel>.xcs` + `RUN.bat` | `[xcs]` v `machine.toml` | SCM Maestro pre CX100; `RUN.bat` prevedie na `.pgmx` cez XConverter na PC pri stroji |
| `nesting/<mat><tabuľa>N.xcs`, `CncRun.txt` | `[nesting]`, `production.mode = "nesting"` | nesting reže obrys a vŕta plochu hore, CX100 robí hrany, drážky a plochu dole |
| `mpr/<diel>.mpr` | `[mpr]` | WoodWOP, alternatíva |
| `labels/*.pdf` | `[labels]` | rozmer a poradie tlače štítkov ako v programe dielne |

Celý formát programov je v šablónach profilu stroja, nový stroj je teda nový súbor TOML. Vzory: [`examples/dielna/machine.toml`](examples/dielna/machine.toml), [`machine-nesting.toml`](examples/dielna/machine-nesting.toml), [`examples/machines/woodwop-vzor.toml`](examples/machines/woodwop-vzor.toml). Podrobnosti v [technickom popise](docs/podrobnosti.sk.md).

## Ovládanie skriptom a agentmi

Každá akcia je **príkaz** (`cabinet.add`, `wall.set`, `front.split`, `project.export_cvj`, …). Pás, panely, klávesové skratky, CLI aj lokálny riadiaci kanál JSON volajú tie isté príkazy, takže všetko, čo sa dá kliknúť, sa dá aj naskriptovať.

```sh
cabinetcraft-app.exe --control 7417 kuchyna.toml      # okno + riadiaci kanál
cabinetcraft serve kuchyna.toml --control 7417        # bez okna
cabinetcraft run kuchyna.toml --script kroky.jsonl    # dávkový skript
cabinetcraft commands                                 # zoznam príkazov
```

```json
{"id":1,"method":"engine.execute","params":{"command":"cabinet.add","params":{"type":"base","w":600}}}
{"id":2,"method":"ui.screenshot","params":{"path":"C:/tmp/snimka.png"}}
```

Protokol: [`docs/control-protocol.md`](docs/control-protocol.md). AI agenti, ktorí pracujú na kóde, začínajú v [`CLAUDE.md`](CLAUDE.md).

## Architektúra

Vrstvy kontroluje `cargo xtask layers`; nič pod vrstvou aplikácií nezávisí od GUI.

| Vrstva | Crate-y | Úloha |
|---|---|---|
| L0 | `cc-model`, `cc-i18n` | typy (projekt, štandard, diely, operácie), preklady |
| L1 | `cc-config`, `cc-generate`, `cc-joints`, `cc-validate`, `cc-room`, `cc-nest`, `cc-import-cvj` | načítanie a kontrola súborov, skrinka → diely, spoje → operácie, kontrola, miestnosť, nesting, čítanie `.cvj` |
| L2 | `cc-out-*`, `cc-scene` | CSV, `.xcs`, `.mpr`, PDF, zápis `.cvj`; 3D scéna a CPU raster |
| L3 | `cc-pipeline` | projekt → model → kontrola → výstupy |
| L4 | `cc-engine` | relácia, príkazy, späť/znova |
| L5 | `cc-control` | riadiaci kanál JSON |
| aplikácie | `cc-app`, `cc-cli` | okno (egui + wgpu) a príkazový riadok |

Konvencie: milimetre, pravotočivé sústavy, plocha **F** dielu je vždy k interiéru, olepená predná hrana je vždy **Y0**. Výstupy sú deterministické – rovnaký vstup dá bajtovo rovnaké programy, strážia to golden testy.

```sh
cargo xtask ci     # fmt, clippy -D warnings, testy, vrstvy, preklady
```

## Preklady

Rozhranie je v **slovenčine** a **angličtine**. Na svoj jazyk stačí textový editor, Rust netreba:

1. skopíruj [`locales/en.toml`](locales/en.toml) ako `<kód>.toml` (`de.toml`, `pl.toml`, `cs.toml`, …),
2. prelož texty v úvodzovkách, kľúče nechaj,
3. daj súbor do priečinka `locales` vedľa programu a vyber ho v *Nástroje* → *Jazyk*,
4. pošli pull request.

Návod a kontrola chýbajúcich kľúčov: [`locales/README.md`](locales/README.md).

## Plán

- [ ] skúška programov CX100 a nestingu na stroji, potom `verified = true`
- [ ] pracovné dosky, lišty, zástena a stropy (rozloženie je, funkcia ešte nie)
- [ ] tvarované diely a šikmé dvierka
- [ ] ručné operácie v editore dosky (CAM)
- [ ] súpis kovania v CSV/PDF, úchytky, kontrola kolízií čiel
- [ ] inštalátor pre Windows a podpísané vydania

## Licencia

Na výber pod jednou z licencií

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT ([LICENSE-MIT](LICENSE-MIT))

Ak výslovne neuvedieš inak, každý tvoj príspevok je pod oboma licenciami bez ďalších podmienok.

CABINET VISION je ochranná známka spoločnosti Hexagon. CabinetCRAFT je nezávislý projekt, nie je spojený s Hexagonom ani ním podporovaný; súbory zákaziek číta a zapisuje iba na spoluprácu s existujúcou inštaláciou. Blum, TIP-ON, Merivobox, SCM, Maestro a WoodWOP sú ochranné známky svojich vlastníkov.

<div align="center"><sub>Vzniklo v malej slovenskej stolárskej dielni, pre stolárske dielne.</sub></div>
