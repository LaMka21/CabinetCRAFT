# CabinetCRAFT – podrobnosti (technický popis)

Generátor korpusových skriniek → diely → CNC programy pre **SCM CX100 (Maestro `.xcs`)**. Prehľad je v [README.sk.md](../README.sk.md); tu je podrobný technický popis formátov, štandardu a výstupov. Kapitoly („kap. 8“) odkazujú na internú špecifikáciu MVP.

## Stav

| Míľnik | Obsah | Stav |
|---|---|---|
| M1 | model, konfigurácia, generátor dielov, rozpis CSV, CLI | ✅ hotové |
| M2 | spoje a operácie (kolíky, drážka, rad dier), `model.json` | ✅ hotové |
| M3 | validácia (kap. 8), profil stroja `machine.toml` | ✅ hotové |
| M4 | WoodWOP `.mpr` (šablóny v `machine.toml`), alternatívny výstup | ✅ hotové, `BohrHoriz`/`Nuten` neoverené |
| M5 | **SCM Maestro `.xcs` pre CX100** + `RUN.bat` pre XConverter | ✅ hotové; čaká na skúšku na stroji (O1) |
| – | **konštrukčný štandard dielne** (`examples/dielna`) | ✅ hotové; kolíky a drážka sedia s programami CV, niektoré hodnoty `[?]` treba potvrdiť |
| – | **okenná aplikácia** `cabinetcraft-app` | ✅ prvá verzia; spúšťa sa, všetky záložky overené snímkou okna |
| F4 | **miestnosť**: steny, okná a dvere, skrinky pri stenách, pôdorys, pohľad na stenu, 3D (`examples/dielna/kuchyna.toml`) | ✅ hotové |

## Aplikácia (okno)

![CabinetCRAFT – 3D náhľad](images/ui-3d.png)

```bash
cargo build --release -p cc-app
```

Spustí sa `target\release\cabinetcraft-app.exe` (jeden súbor, nič sa neinštaluje). Pri prvom spustení otvorí vzorový projekt `examples/dielna`, potom vždy naposledy otvorený projekt. Projekt sa dá zadať aj ako argument: `cabinetcraft-app.exe cesta\project.toml`.

- **Ľavý panel:** názov projektu, materiály, konštrukčný štandard a skrinky. Skrinku môžeš pridať (spodná, horná, vysoká), duplikovať, odstrániť alebo presunúť a meniť jej rozmery a police. Model sa pri každej zmene hneď prepočíta.
- **3D náhľad:** ľavým tlačidlom myši sa otáča, pravým posúva, kolieskom približuje a kliknutím sa vyberie diel. Zadné steny, police a vŕtanie sa dajú skryť a diely sa dajú rozložiť od seba.
- **Diely:** rozpis s rozmermi, olepením, počtom operácií a názvom programu.
- **Výkres dielu:** pohľad na plochu F s kolíkmi, radom dier, vŕtaním do hrán a drážkou. Pod ním je tabuľka operácií a vygenerovaný program `.xcs`.
- **Kontrola:** chyby a varovania validácie (kap. 8). Klik na diel otvorí jeho výkres.
- **Štandard:** hodnoty konštrukčného štandardu sa dajú meniť a hneď vidieť v modeli. Pri uložení sa prepíšu iba zmenené hodnoty a komentáre `[P]`/`[U]`/`[?]` v súbore ostanú.
- **Export pre stroj:** zapíše `cutlist.csv`, `model.json`, `xcs/*.xcs` a `RUN.bat` do zvoleného priečinka (predvolene `out` vedľa projektu). Ak kontrola našla chyby, export je zablokovaný.
- **Nový / Uložiť ako:** nový projekt prevezme materiály, štandard a stroj z otvoreného projektu. Pri uložení do iného priečinka sa tam skopírujú `materials.toml`, `standards/…` a `machine.toml`.

## Miestnosť

Projekt môže mať miestnosť (špecifikácia kap. 13, vzor `examples/dielna/kuchyna.toml`):

```toml
[room]
wall_height = 2600
toe_height = 100        # sokel: spodok spodných a vysokých skriniek
upper_top = 2200        # horná hrana horných skriniek

[[wall]]                # vnútorná plocha steny; miestnosť je napravo od smeru
id = "W1"
x = 0
y = 2800
length = 3600
angle = 0

[[opening]]
wall = "W1"
kind = "window"         # window | door | opening
at = 1250
width = 900
height = 1200
elevation = 1000

[[cabinet]]
id = "SP2"
type = "base"
w = 900
h = 762
d = 510
wall = "W1"             # skrinka pri stene
at = 1250               # ľavá hrana od ľavého konca steny
```

- **Pôdorys:** kreslenie stien klikaním (prichytenie ku koncom stien, 45°, 10 mm), obdĺžniková miestnosť jedným tlačidlom, okná a dvere, posun skriniek pozdĺž steny s prichytením k susedom, vyplnenie steny spodnými alebo hornými skrinkami, kóty zvonku.
- **Pohľad na stenu:** z miestnosti, s kótami radov, dĺžky steny a výšok.
- **3D:** steny s otvormi a podlaha; stena medzi kamerou a miestnosťou sa skryje.
- Kontrola hlási ako varovanie presah steny, prekryv skriniek (aj v rohu), zásah do okna alebo dverí a skrinku nad stropom. Výrobu neblokuje.

## Štítky dielov

Export pre stroj zapíše aj `labels/stitky.pdf` (strana = štítok 100 × 80 mm pre tlačiareň štítkov) a `labels/stitky-A4.pdf` (6 štítkov na A4 s rámikom na strihanie). Obsah štítku podľa štítkov, ktoré dielňa tlačila z CV:
- zákazka, číslo skrinky (veľké), materiál a hrúbka,
- „n: meno dielu“ (poradie v skrinke), ID dielu, rozmer a olepené hrany,
- náčrt dielu: olepené hrany hrubou čiarou, diery na ploche so štítkom plné, na opačnej prázdne krúžky, kolíky v hranách, drážka (na opačnej ploche čiarkovane), kóty L × W, značka prednej hrany Y0,
- pri nestingu tabuľa, plocha hore a **Up/Down** (Down = pred CX100 diel otočiť),
- meno programu CX100 a jeho **čiarový kód Code 39**; diel bez práce na CX100 má „bez programu CX100“.

Písmo je Helvetica (WinAnsi). Znaky č, ľ, ť, ň, ď, ĺ, ŕ sa na štítku píšu bez diakritiky.

## Príkazový riadok

```bash
cargo run -p cc-cli -- build examples/dielna/project.toml --out out
```

`examples/dielna` je štandard dielne pre CX100. `examples/s1` je príklad zo špecifikácie a používajú ho akceptačné testy.

Prepínače:
- `--merge` zlúči rovnaké diely do jedného riadku,
- `--decimal-comma` použije desatinnú čiarku,
- `--machine <súbor>` určí profil stroja; predvolene sa použije `machine.toml` vedľa projektu, ak existuje.

Pred zápisom výstupov beží validácia (kap. 8). **Chyby zastavia výstup** (návratový kód 1), varovania sa iba vypíšu. Bez profilu stroja sa kontroly rozmerov stroja a vrtákov vynechajú. `examples/s1/machine.toml` je iba vzor a hodnoty treba nahradiť údajmi skutočného stroja.

Výstup: `out/cutlist.csv`, `out/xcs/<diel>.xcs` + `out/xcs/RUN.bat` (ak má profil stroja sekciu `[xcs]`), `out/mpr/<diel>.mpr` (ak má sekciu `[mpr]`) a `out/model.json` (celý model vrátane operácií v súradniciach dielu). Varovania, napríklad dotyk dielov bez pravidla spoja, sa vypisujú na stderr.

## Štruktúra

| Crate | Úloha |
|---|---|
| `cc-model` | typy: projekt, materiály, štandard, diely, transformácie, operácie |
| `cc-config` | načítanie `project.toml`, `materials.toml`, `standards/<meno>.toml` a ich kontrola |
| `cc-generate` | skrinka → diely (špecifikácia, kap. 5) |
| `cc-joints` | spoje → operácie: kolíky, drážka zadnej steny, rad dier (kap. 6) |
| `cc-validate` | validácia geometrie a obrábania (kap. 8) |
| `cc-nest` | nesting: rozloženie dielov na tabule (MaxRects + žíhanie), plocha hore, delenie práce nesting / CX100 |
| `cc-out-csv` | rozpis dielov (kap. 7.1) |
| `cc-out-xcs` | programy SCM Maestro `.xcs` pre CX100 + `RUN.bat`, riadené šablónami z `machine.toml` (kap. 7.4); modul `nest`: programy tabúľ pre SCM nesting + `CncRun.txt` |
| `cc-out-mpr` | programy WoodWOP `.mpr` riadené šablónami z `machine.toml` (kap. 7.3, alternatíva) |
| `cc-out-util` | spoločné pomôcky výstupov (čísla, šablóny, ASCII) |
| `cc-room` | miestnosť: steny, poloha skrinky pri stene, kontrola umiestnenia, vyplnenie steny, kóty (kap. 13) |
| `cc-import-cvj` | import zákaziek CABINET VISION `.cvj` (iba čítanie; rozpracované) |
| `cc-out-pdf` | štítky dielov v PDF (100 × 80 mm a A4), čiarový kód Code 39 |
| `cc-pipeline` | celý beh projekt → model → kontrola → zápis výstupov (spoločné pre CLI aj aplikáciu) |
| `cc-cli` | príkaz `cabinetcraft build` |
| `cc-app` | okenná aplikácia `cabinetcraft-app` (egui): zadanie, 3D náhľad, diely, výkres, kontrola, štandard, export |

Konvencie (kap. 3):
- mm, pravotočivé sústavy,
- plocha **F** dielu je vždy k interiéru,
- predná (olepená) hrana dielu je vždy **`Y0`**.

## Konštrukčný štandard dielne

Súbor `examples/dielna/standards/dielna-cx100.toml`. Pri každej hodnote je uvedený zdroj: `[P]` znamená z programov CX100, ktoré vyrábal CABINET VISION, `[U]` potvrdil používateľ a `[?]` je odhad na overenie v dielni.

| Oblasť | Nastavenie |
|---|---|
| Hrúbky | korpus DTDL 18, zadná stena HDF 3 (`materials.toml`) |
| Zadná stena | drážka pílkou jedným prechodom, os 18,5 mm od zadnej hrany, hĺbka 9, cez celý diel |
| Kolíky | Ø8, hĺbka 13 do plochy a 26 do hrany. Rozmiestnenie `layout = "pairs"`: dvojice 32/96 od konca, ďalšie dvojice rovnomerne s rozstupom najviac 250 mm. Na krátky spoj (napr. vlys 128) idú 2 kolíky 18 mm od koncov. `stop_at_back = false` dáva kolíky po celej hĺbke dielu ako v CV. |
| Rad dier | Ø5, hĺbka 10. Rozmiestnenie `layout = "per_shelf"`: 3 diery s roztečou 32 pri každej polici, stredná je najbližšie k spodku police. |
| Pílka | `LAMA`, šírka rezu 4,0 mm (`machine.toml`, `[xcs.slot]`) |

Polohy kolíkov a drážky test `crates/cc-out-xcs/tests/dielna.rs` porovnáva s reálnymi programami: dno hornej skrinky s `966f0040`, dno spodnej skrinky s `966f0020` a vlys s `966f0004`.

**Na potvrdenie `[?]`:** vôle police (1 mm na stranu, 20 mm spredu), vôľa zadnej steny v drážke (1 mm), odsadenie radov dier (37 mm spredu aj zozadu), najnižšia a najvyššia diera (64 mm), hrúbka ABS a pracovný rozsah CX100 (`work_area`, `min_part`).

## SCM Maestro `.xcs` (CX100)

Profil je v `examples/dielna/machine.toml` (rovnaký formát má `examples/s1/machine.toml`), sekcia `[xcs]`. Formát je prevzatý z programov, ktoré pre CX100 vyrábal CABINET VISION (vzory bez názvu zákazky sú v `tests/fixtures/scm-cx100/`).

- **Hlavička:** `SetMachiningParameters` (CX100), `CreateFinishedWorkpieceBox("Dilec",L,W,T)`.
- **Vŕtanie:** `SelectWorkplane` + `CreateDrill`. Strana dielu sa mapuje na rovinu: F → `Top`, X0 → `Left`, X1 → `Right`, Y0 → `Front`, Y1 → `Back`. Bočné roviny sa merajú pri pohľade zvonka, napríklad `Back = L − x` a `Left = W − y`.
- **Drážka:** `CreateSegment` + `CreateSlot` pílkou `LAMA`. Drážka širšia ako rez sa robí viacerými prechodmi s krokom `max_step` a drážka cez celý diel sa predĺži o 60 mm.
- **`RUN.bat`:** na PC pri stroji prevedie `.xcs` na `.pgmx` cez `XConverter.exe` s `COLL.tlgx`. Cesty sú v `[xcs.run_bat]`.
- **Test zhody** (`crates/cc-out-xcs/tests/conformance.rs`) overuje, že výstup používa iba funkcie, počty argumentov a roviny z vašich reálnych programov.

**Pred prvým rezom over na stroji:**
1. Skopíruj `out/xcs/*` do `C:\Vyroba\CX100` a spusti `RUN.bat`. Ak XConverter `.pgmx` vytvorí, formát prešiel.
2. Na skúšobnom diele skontroluj:
   - počiatok roviny `Top` (O1),
   - smer osi v `Left`/`Right`,
   - šírku drážky zadnej steny (rez 4,0 mm + viac prechodov, ak je drážka širšia).

Na vývojovom PC Maestro nemá licenciu, preto sa XConverter tam spustiť nedá: skončí s kódom 1 aj na pôvodných programoch z CV.

## SCM nesting + CX100 (`production.mode = "nesting"`)

Predvolený režim `cx100` je doterajší postup (diely z píly, všetko obrábanie na CX100). Režim `nesting` zapne sekcia `[production] mode = "nesting"` v profile stroja; vzor je `examples/dielna/machine-nesting.toml` (zhodný s `machine.toml` okrem režimu):

```
cargo run -p cc-cli -- build examples/dielna/project.toml --machine examples/dielna/machine-nesting.toml --out out
```

Výstup: `out/nesting/<mat><tabuľa>N.xcs` (program tabule), `out/nesting/CncRun.txt` (súpis behu pre obsluhu, UTF-16), `out/xcs/*.xcs` + `RUN.bat` iba so zvyšnou prácou pre CX100. Formát programov tabúľ je v sekcii `[nesting]` (šablóny podľa programov tabúľ dielne).

- **Rozloženie:** tabuľa 2800 × 2070 (iný rozmer pre materiál: `[[nesting.sheet]]`), okraj 10, medzera 14,2. Diely sa skupinujú podľa materiálu a hrúbky. Diel bez vlákna sa otáča voľne, s vláknom po dĺžke nie, s vláknom po šírke vždy o 90°. Materiály v `skip_materials` (HDF) idú na pílu.
- **Delenie práce:** nesting robí obrys a vŕtanie plochy, ktorá je hore. CX100 robí vŕtanie do hrán, všetky drážky (pílou) a plochu, ktorá je dole. Diel bez zvyšnej práce nemá program CX100.
- **Plocha hore:** menej programov CX100, potom viac dier na nestingu, pri zhode F. Diel s plochou B hore je na tabuli zrkadlený (`x' = L − x`).
- **Poradie na tabuli:** vŕtanie po nástrojoch (`drill.order`), v skupine najkratšia cesta; onion skin malých dielov (T − 2); obrys každého dielu (T + 0,2) najprv malé, potom veľké, v smere hodín s korekciou vľavo, nábeh 7 mm z výšky −10, prekrytie 12; odrezanie zvyšku tabule; parkovanie.
- **Mená:** tabule `0101N.xcs` (`{mat}{sheet}N`), programy CX100 podľa ID dielu ako v režime `cx100`.
- Všetky šablóny `[nesting]` majú `verified = false`: pred prvým rezom ich treba skontrolovať v Maestre pri nestingu.

## WoodWOP `.mpr` (alternatíva)

Formát je v sekcii `[mpr]` profilu stroja (vzor `examples/machines/woodwop-vzor.toml`). Hlavička `[H`, obrobok, komentár a makrá `BohrVert`, `BohrHoriz` a `Nuten` sú zoznamy `[pole, šablóna]` s premennými, napríklad `["XA", "{x}"]`. Neznáme premenné odmietne už načítanie profilu.

- `verified = true` (`BohrVert`, hlavička): polia zodpovedajú formátu z analýzy.
- `verified = false` (`BohrHoriz`, `Nuten`): treba ich **overiť v editore woodWOP alebo na stroji** (otázka O2). CLI pri ich použití vypíše upozornenie.
- Smer vŕtania do hrany (`XP/XM/YP/YM`) je v `[mpr.bohr_horiz.dir]`. Mapovanie hrán na stroj treba tiež overiť (O1).
- Súbory sú ASCII s CRLF a diakritika sa prepíše. Diel s operáciami na ploche B dostane navyše `<diel>_B.mpr`, otočený okolo osi y.
- Pred každým behom sa zmažú staré súbory `.mpr` v `out/mpr/`.

## Testy

```bash
cargo test --workspace
```

Akceptačný test z kap. 10 sa nachádza v `crates/cc-generate/tests/spec_example.rs`, operácie v `crates/cc-joints/tests/spec_example.rs`, golden súbory v `tests/golden/s1/` (`cutlist.csv`, `model.json`). Pravidlá validácie majú každé vlastný test v `crates/cc-validate/tests/rules.rs`. Golden programy `.xcs`/`RUN.bat` sú v `tests/golden/s1/xcs/` a `tests/golden/dielna/xcs/`, `.mpr` v `tests/golden/s1/mpr/`. Porovnávajú sa bajt po bajte a v gite sú vedené ako binárne. Po zámernej zmene výstupu ich treba pregenerovať príkazom `build` a rozdiel skontrolovať.
