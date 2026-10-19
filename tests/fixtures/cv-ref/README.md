# Referenčná zákazka REF2026 (CABINET VISION 2026) – dataset pre test parity (W6)

Odvodené údaje (nie súbory CV) zo zákazky dielne **REF2026** (kuchyňa, 2 steny, 13 skriniek).
Slúžia ako referencia pre `xtask parity`: rozmery, poloha a obrábanie každého dielu tak, ako ich vyrobil CV.

## Pôvod

| Zdroj | Čo z neho je |
|---|---|
| súbor zákazky `.cvj` (CV 2026, prúd `Version` = 2026 / 1 / 1515, uložený 16. 10. 2026) | skrinky, diely (`NCID`), rozmery, polohy, olepenie (`BAND`), atribúty, otvory čiel |
| programy SCM z 25. 9. 2026: beh 966 (nesting `9660301N`–`9660502N` + CX100 `966b/f*`), 967 (píla, `967f0057`), 968 (pracovná doska, `9680601N`, `968f*`) | všetky diery, drážky a frézovania |
| `CncRun966/967/968.txt` | materiál tabúľ a dielov |
| `966.plp`, `967.plp`, `968.plp`, `.ard` tabúľ | ktoré diely sú na ktorej tabuli, stred dielu na tabuli |

Číta sa iba nešifrovaný prúd `Contents` (formát: `findings/07-cvj-format.md`), postup rekonštrukcie
tabúľ podľa `findings/08-nesting-scm.md`, mapovanie dielov cez `NCID` podľa `findings/09-kovanie.md`.
Skripty (Python stdlib, iba čítanie): interné `tools/parity_ref/` (mimo repa) (`build_dataset.py <priečinok>` zostaví
tento priečinok, `check.py <priečinok>` urobí kontroly). Zostavené 2026-10-16.

**Anonymizácia:** dataset neobsahuje meno, adresu ani telefón zákazníka, popis zákazky ani cesty k súborom.
Zostáva iba číslo zákazky `REF2026`, mená katalógových položiek a kódy dielov. Objekty mimo skriniek
(model steny s menom zákazníka, spotrebiče, obrazy) sú vynechané. Kontrola: hľadanie priezviska zákazníka
(bez diakritiky, bez ohľadu na veľkosť písmen) v celom priečinku nič nenájde.

## Súbory

| Súbor | Riadkov | Obsah |
|---|---|---|
| `parts.csv` | 95 | všetky diely s `NCID` (0001…0095) všetkých skriniek a pracovných dosiek |
| `ops.csv` | 1002 | každá diera, drážka pílou a frézovaná dráha v súradniciach dielu (693 nesting + 307 CX100 + 2 trasy na tabuliach) |
| `cabinets.csv` | 13 | skrinky s `CABNO` 1…13: rozmery, poloha, atribúty, čelá |

Formát: UTF-8, oddeľovač `;`, desatinná bodka, mm, uhly v stupňoch, riadky LF. Prázdne pole = údaj neexistuje.

## Súradnice

### Rámec dielu (hlavný, = rámec CV)
- Diel je kváder `[0, L] × [0, W] × [0, T]` vo **vlastnom lokálnom rámci CV**: `L` = rozmer pozdĺž osi `x` dielu
  (parameter CV `DX`), `W` pozdĺž `y` (`DY`), `T` hrúbka pozdĺž `z` (`DZ`). `L` nemusí byť dlhší rozmer.
- Pozor: `CncRun` a štítky CV uvádzajú rozmer opačne (`L` = `DY`, `W` = `DX`); program CX100 má `"Dilec"` box `DY × DX × DZ`.
- Plochy: `z0` (z = 0), `zT` (z = T); hrany `x0`, `x1` (x = L), `y0`, `y1` (y = W).
- `pos_x/y/z` = počiatok dielu, `rot_x/y/z` = uhly `AX, AY, AZ` v **ráme skrinky CV** (`frame = cabinet`):
  počiatok v **ľavom zadnom dolnom rohu skrinky vrátane sokla** (podlaha), `+X` doprava, `+Y` hore, `+Z` dopredu
  (do miestnosti). Matica dielu `M = T(pos) · Rz(rot_z) · Ry(rot_y) · Rx(rot_x)` (stĺpcové vektory). Stĺpec `axes`
  to isté slovami (napr. `x+Z y+Y z-X` = os x dielu ide v skrinke dopredu…). Pracovné dosky (`frame = counter`)
  sú v ráme pracovnej dosky CV.
- Korpus spodných skriniek začína na `Y = 100` (`toe_h`), čelá a vnútro sú v skupinách posunutých o 100.

### Náš rámec (`our_*`, SPECIFIKACIA §3.2–3.3)
- `z_our` ide k interiéru skrinky (`face_F_is` = ktorá plocha CV je naša **F**, `z0` alebo `zT`),
- `y_our` ide od prednej hrany dozadu (`front_edge_is` = ktorá hrana CV je naša **Y0**); diel bez prednej hrany
  (chrbát, čelo, zvislý vlys, blendy) má `y_our` zdola nahor (Y0 = spodná hrana),
- `x_our = y_our × z_our` (pravotočivý), počiatok tak, aby súradnice boli 0…`our_L`/`our_W`/T.
- Toto pravidlo dáva presne rámce generátora CabinetCRAFT pre boky, dno, vrch/spojky a chrbát.
  `our_x_is`/`our_y_is` hovoria, na ktorú os CV ide naša os (napr. `-y`). Pri dieloch mimo korpusu
  (sokel, blendy, LED rampa, pracovná doska: F = horná plocha) je „interiér“ odvodený od stredu skrinky – berte ich opatrne.
- **Polohovateľná polica (`AS`)** nemá interiér na jednej strane. Platí konvencia generátora CabinetCRAFT
  (`cc-generate`, rola `AdjustableShelf`): `z_our` = **hore**, teda **F = horná plocha**, `y_our` od čela dozadu,
  `x_our` doprava; diery čapov podperky (Ø5 × 10 zospodu) sú na **B**. Do 2026-10-16 bral dataset F podľa stredu
  skrinky (raz `z0`, raz `zT`, pri polici v strede prázdne); opravené pri všetkých 9 policiach (`face_F_is = zT`).

## `parts.csv`

| Stĺpec | Význam |
|---|---|
| `ncid` | číslo dielu v CNC výstupoch (`966b0006` → 6) |
| `cabno`, `cabinet_name` | číslo a katalógová položka skrinky (`Counter` = pracovná doska) |
| `part_code`, `part_name`, `pid` | kód (LU, RU, UB, TS, TO, DE, BO, AS, LF, RF, FB, S_DSLAB = doska čela, BBT/BBK = dno/zadok zásuvky, CT), názov a `PID` z CV |
| `class` | trieda skrinky CV (Base / Upper) |
| `path` | cesta v strome skrinky (`Case/LU`, `Face/DWR_OPEN/DBOX/BBT` …) |
| `L`, `W`, `T` | rozmery v ráme dielu (pozri vyššie) |
| `material` | materiál z `CncRun` tabule/behu (966: tabuľa, 967: HDF, 968: PD) |
| `edge_X0`…`edge_Y1` | kód olepenia hrany v ráme CV (znak parametra `BAND`: poradie hrán y0, x1, y1, x0; `N` = bez olepenia, prázdne = znak chýba = bez olepenia). Tvarové diely (`band_raw` > 4 znaky) majú hrany prázdne |
| `band_raw` | pôvodný reťazec `BAND` |
| `frame`, `pos_*`, `rot_*`, `axes` | poloha v ráme skrinky CV |
| `face_F_is`, `front_edge_is`, `our_L`, `our_W`, `our_x_is`, `our_y_is`, `our_edge_*` | náš rámec |
| `run` | beh S2M (966 nesting, 967 píla, 968 pracovná doska; prázdne = diel sa nevyrába) |
| `cx100_program`, `cx100_face_up`, `cx100_rot180` | program CX100, plocha hore na CX100 (`f` = `zT`, `b` = `z0`), otočenie rámca (nižšie) |
| `label_updown` | text na štítku: `Down` = diel sa medzi nestingom a CX100 otáča (08 §3.3) |
| `nest_sheet`, `face_up_nest`, `our_face_up_nest` | tabuľa a plocha hore na nestingu |
| `nest_rot`, `nest_rect` | otočenie (°, proti smeru hodín) a obdĺžnik dielu na tabuli `x0 y0 x1 y1` |
| `nest_fit` | istota orientácie: `zhoda/dier (alt …)` = koľko dier tabule sedí s predikciou z `.cvj` pri zvolenej / najlepšej inej orientácii; `default(no face ops)` = diel bez dier v ploche (orientácia neovplyvní dáta); `route` = podľa drážky/výrezu |

## `ops.csv`

| Stĺpec | Význam |
|---|---|
| `ncid`, `source` | diel; `nest` (tabuľa) alebo `cx100` |
| `program`, `line`, `line_end` | program `.xcs` a riadok (`CreateDrill`, prvý a posledný riadok dráhy/rezu) |
| `kind` | `P` plochý vrták, `L` hrot (značka), `E008` diera frézou Ø12, `saw` drážka pílou (`LAMA`), `route` frézovaná dráha |
| `tool` | nástroj z programu (`-1` = automaticky podľa priemeru) |
| `machine_face` | rovina stroja (`Top`, `Front`, `Back`, `Left`, `Right`) |
| `cv_face`, `our_face` | plocha/hrana dielu: CV `z0/zT/x0/x1/y0/y1`, naša `F/B/X0/X1/Y0/Y1` |
| `x`, `y`, `z` | bod vstupu nástroja v ráme dielu CV (pri ploche `z` = 0 alebo T, pri hrane výška v hrúbke) |
| `diameter`, `depth` | Ø a hĺbka do materiálu; pri dráhe `depth` = najväčšia hĺbka |
| `width`, `x2`, `y2` | drážka pílou: šírka (pílový kotúč 4 mm, viac rezov = viac riadkov) a koniec rezu (rez presahuje diel – nábeh/výbeh) |
| `comp`, `path` | dráha: korekcia (1 vľavo, 0 os) a vrcholy `x:y:hĺbka` (−10 = nad materiálom, nábeh) |
| `our_x`, `our_y`, `our_z`, `our_x2`, `our_y2` | to isté v našom ráme |
| `note` | pri dierach v prekrytí obdĺžnikov tvarových dvierok 0032/0059: komu boli priradené |

Obrysy dielov na tabuli (spojený obrys, onion skin, odrezanie zvyšku) v `ops.csv` nie sú: sú to hrany `L × W` dielu.

## `cabinets.csv`

Rozmery, poloha a atribúty skrinky ako v CV (mená stĺpcov = mená parametrov CV). Pozor na tieto stĺpce:

| Stĺpec | Význam |
|---|---|
| `Spojovacie_kovanie_zap` | spojenie s **ľavou** susednou skrinkou (diery Ø5 × 16 v ploche F ľavého boku a v pravom boku suseda, findings 12 §8): 0 vypnuté, 1 zapnuté, 2 zapnuté s ručnými polohami; prázdne = skrinka nemá ľavý bok LU/LF. V CV je to parameter ľavého boku (`Spojovacie_kovanie_zap_2`, pri inom nastavení dielne `_zap_1`), nie skrinky. Do 2026-10-16 bol stĺpec `Spojovacie_kovanie_zap_1` zo skrinky (vždy prázdny). |
| `Posun_vrtania` | posun zhodnej diery v susedovi (mm), iba pri zapnutom spojení |

## Ako vznikli súradnice (pre kontrolu)

- **CX100** [P/O]: box programu `L_cx × W_cx = DY × DX`. Pre plochu `f` (hore `zT`): `x_cx = y`, `y_cx = DX − x`;
  pre `b` (hore `z0`): `x_cx = DY − y`, `y_cx = DX − x`. Bočné roviny: `Front` (y_cx = 0) súradnica = `x_cx`,
  `Back` = `L_cx − x_cx` (overené na asymetrických dierach 0073, 0019, 0027), `Left` = `W_cx − y_cx`, `Right` = `y_cx`
  (v tejto zákazke symetrické, [O]); druhá súradnica = vzdialenosť od hornej plochy (všetky hrany sú v strede hrúbky).
  Pri 6 dieloch (všetky s osou `z` dielu = `+X` skrinky: pravé boky RU/RF, lišta blendy, bočný sokel) CV rámec
  programu otočil o 180° v rovine dielu (`cx100_rot180 = 1`, potvrdené zhodou dier s `.cvj`: 0026, 0037, 0041, 0048, 0061, 0031).
- **Nesting** [O]: obdĺžnik dielu z dráhy spojeného obrysu + stredu zo štítka; plocha hore a otočenie (4 možnosti)
  zvolené najlepšou zhodou dier tabule s polohami odvodenými z `.cvj` (spojovacie kovanie `HW1/HW2` na hranách,
  misky `HNG`, podložky, výsuvy, nožičky, `LBORE`, diery do hrán z CX100, príchytky čiel, sedlá podperiek).
  Pre diely s programom CX100 vyšla plocha vždy v súlade s pravidlom Up/Down zo štítkov.

## Známe medzery

1. `.cvj` je uložený 16. 10., CNC programy sú z 25. 9. Všetky diery sedia s rozmermi a kontrolami nižšie, ale
   zmena zákazky medzi týmito dátumami sa vylúčiť nedá.
2. Diel 0074 (`BO ZASUVKA`, 100 × 250 × 4, značka elektrickej zásuvky) nie je v žiadnom behu: bez materiálu a operácií.
3. Materiál je z behu S2M, nie z `.cvj` (materiály sú mimo prúdu `Contents`). Diely 0091 a 0095 (38 mm, beh 968
   bez programu) majú materiál pracovnej dosky odvodený z behu [O].
4. Tvarové diely (0029, 0030, 0032, 0058, 0059, 0028/0056 šikmé boky) majú v `parts.csv` iba obalový kváder;
   tvar obrysu nie je v datasete.
5. Orientácia na nestingu pri 29 dieloch bez dier v ploche je predvolená (plocha podľa CX100/štítka, inak `zT`);
   súradnice ich operácií to neovplyvní. Pri policiach AS sú 4 diery sedla podperky symetrické; plocha `z0`
   (spodná, naša B) je predpoklad podľa podperky Sekura 7 [O].
6. Objekty bez `NCID` nie sú v `parts.csv`: úchytkové profily UK16 (`S_BEAD`, `_NOCNC`), náhrada varnej dosky
   `BNDIN`, deliace čiary čiel `MR`/`MS` (šírka 0), kovanie (`MODEL`, `HNG` …), spotrebiče a model steny.
7. `REVL/REVR/REVT/REVB` sú v `cabinets.csv` iba tam, kde ich skrinka prepisuje; prázdne = predvolené zo štandardu
   (09 §4.2: bok 1, spodné vrch 2, spodok 0).
