# Parita s CABINET VISION (dielňa CASTEL)

Cieľ: CabinetCRAFT robí pre dielňu to isté, čo jej CV 2026: rovnaké skrinky, diely, rozmery,
obrábanie a súpisy. Zdroj pravdy sú programy a súbory zákaziek dielne (REF2026, test_1),
rozbor formátov a výstupov CV je v interných poznámkach `findings/` mimo repa (clean-room).

Stav: ✅ hotové a overené proti CV · 🟡 čiastočne / iné ako CV · ❌ chýba.
Overenie celku: referenčná zákazka REF2026 diel po diele (W6): `cargo xtask parity --out sprava.md`
(cesta k `.cvj` v `CABINETCRAFT_CVJ_REF`, dataset `tests/fixtures/cv-ref/`).

**Stav parity REF (2026-10-16):** diely 79/95 spárované, rozmer sedí 45; diery 489/982 sedia, u nás navyše 297.
Bežné skrinky S_KUCH_D (11, 13) a S_KUCH_NASTENNA (8) nemajú žiadnu dieru navyše; spodným chýbajú iba
značky nožičiek Ø4 × 2. Najviac rozdielov majú šikmé a rohové skrinky,
drez (zvislý vlys CHRBAT), VARNA (znížené vlysy) a nika.

## 1. Miestnosť a umiestnenie
| Oblasť | Stav | Pozn. | Zdroj |
|---|---|---|---|
| Steny, okná, dvere, pôdorys, pohľad na stenu | ✅ | F4 | 06 |
| Poloha skrinky (od ľavého konca, výška, odsadenie) | ✅ | | 06 §4 |
| Clearance / End / Center Position, Lock Width, uhol | 🟡 | iba „od ľavého konca“; zámok šírky a uhol chýbajú | 11 §4 |
| Horné skrinky „Above Elevation 1518“ | 🟡 | máme `upper_top` miestnosti | 11 §4.1 |

## 2. Korpus
| Oblasť | Stav | Pozn. | Zdroj |
|---|---|---|---|
| Boky, dno medzi bokmi, kolíky v dvojiciach | ✅ | vzor dvojíc overený na CX100 | 01, 05 |
| Spodné: chrbát naložený zozadu (W − 2 × 759), boky a dno D − 4 | ✅ | parita REF; CV počíta chrbát 4 mm, materiál 3 HDF; upevnenie sponkami (vlastník) | 10 §2.4, 12 §1 |
| Spodné: dno a vrch 0,5 od koncov bokov (kolíky 9,5 / 752,5) | ✅ | `base_end_inset`, parita REF | 12 |
| Spodné: vrch TO pri dvierkovej, 2 vlysy pri zásuvkovej/drezovej | ✅ | `top_style` pri skrinke a v katalógu | 10 §2.4 |
| Horné: TO 19 (MDF) na bokoch, boky D − 20,5 pred chrbtom, chrbát W − 2 v drážkach dna a vrchu | ✅ | parita REF skr. 8: všetky diery sedia | 12 §1, §3 |
| Horné: blenda pri skrinke (lišta, čelo blendy 60, dno blendy 50 × 19, vrch cez blendu) | ✅ | `[cabinet.filler]`, `[filler]` v štandarde; parita REF skr. 9: v nestingu 96/96 dier, pri CX100 sú čelo 60 a dno 50 pod minimom stroja → import vynechá kolíky blendy | 12 §2 |
| Horné: LED rampa 19 MDF pod skrinkami (drážka 14 × 13) | ❌ | rozmery a poloha v findings 12 §2 | 12 §2 |
| Drez bez chrbta, zadný vlys „CHRBAT“ zvislo | 🟡 | bez chrbta ✅, zvislý vlys chýba | 11 §5.4 |
| Rad dier políc 53 / 50 od hrán, stredná 6 pod policou (Sekura 7) | ✅ | test: 334/366/398 ako S_KUCH_D | 11 §1.7 |
| Polica: hĺbka − 3, dĺžka − 1, rovnaké otvory, Ø5 × 10 zospodu pre čap podperky | ✅ | parita REF | 10 §2.3–2.4, 12 |
| Sokel 97, zadná plocha 68 za čelom, jeden kus na zostavu | ✅ | 19 MDF (`[plinth] material`), zostava = susedné spodné/vysoké skrinky pri stene s rovnakou prednou líniou a výškou (`runs`), dlhšia ako `max_length` sa delí na styku skriniek; import z CV prenesie ručnú polohu a dĺžku (`plinth_span`, REF: 2680 od X −60, 1944 od X −1372). Výška DE.Y − 3,5 = osadenie − 3 (dno 0,5 nad spodkom boku) | 11 §5.5, 12 §6 |
| Bočný sokel s 2 kolíkmi | ✅ | pod bokom skrinky na odkrytom konci zostavy (nie pri susednej skrinke, v rohu, pri krycom boku), Ø8 × 26 do čela na 18 / 79 a Ø8 × 13 do zadnej plochy predného sokla (`[plinth] side`, `side_dowel`); import `side_plinth_left/right/height` | 11 §5.5, 12 §6 |
| Bok slepého rohu o 3 vpred | ❌ | | 11 §5.2 |
| Šikmá skrinka (SIKMA, REF skr. 1 a 6): `slant` pri skrinke, šikmý bok MDF 19, dno a vrch po jeho vnútornú plochu, pohľadový chrbát `FB` na kolíkoch 3/3/3 + konfirmáty, horná bez vrchu s chrbtom v drážke šikmého boku | 🟡 | rozmery dielov skr. 1 ako CV; tvar (zrezané konce, šikmé hrany, tvarové dvierka) iba obálka; vrch 164,5 je pod VZOR `min_part` 200 → import skrinku zjednoduší na rovnú; pánty šikmých dvierok (100/660, ručné HNGT/HNGB) a dno skr. 6 (CV 363,55 zasahuje 1,16 do boku) inak | 12 §5, dataset cv-ref |

## 3. Spoje (atribúty)
| Oblasť | Stav | Pozn. | Zdroj |
|---|---|---|---|
| Spoje áno/nie, kolíky / kolíky + konfirmát / kolíky + vrut | ✅ | „podľa zákazky“ (vlastník); diera konfirmátu do hrany – rozmer neznámy | 11 §1.3 |
| Vzor 3/3/3 CV (pásma podľa dĺžky hrany), konfirmát iba cez bok | ✅ | `layout = "cv333"`; vzor z dĺžky hrany dielu (dno 330 pri boku 309,5) | 11 §1.3 |
| Excenter, VB35/36, Blum spojka, MaxiFix | ❌ | dielňa ich nepoužíva (REF) | 11 §1.3 |
| Spojovacie kovanie skriniek Ø5 × 16 (výška 80 / H − 80, 100 od čela a zadu; horné v strede hĺbky) | ✅ | atribút „Spojiť s ľavou“ (`link_left`, ručné polohy `link_holes`, import z `.cvj`); protikus v pravom boku suseda; parita REF: všetkých 28 dier; `link_shift` sa zatiaľ nepoužíva | 09 §5.4, 11 §1.9, 12 §8 |

## 4. Čelá a kovanie
| Oblasť | Stav | Pozn. | Zdroj |
|---|---|---|---|
| Dvierka, dvojité, zásuvky, slepé čelo, panel, stĺpce | ✅ | | 09 §4, 11 §3 |
| Škáry pri skrinke (Reveals: hore, dole, boky, medzi) | ✅ | test: skrinka 6 v REF (čelá 600 × 150/303/303, korpusy M/E/E) | 09 §4.2, CV Properties → Reveals |
| Úpravy rozmeru čela pri otvore (Size Adjustments OLL/OLR/OLT/OLB) | ✅ | `adjust` pri čele (vľavo/vpravo/hore/dole), panel „Úprava čela“; test: vpravo +60 ako blenda, hore −30 | 09 §4.2 |
| Úchytka UK16 (OLT −30), presah horných 19 | ✅ | | 09 §4 |
| Pánt Blum 110° INSERTA, počet podľa CV, podložka 20/52 | ✅ | | 09 §1–2, 10 §5 |
| Slepý pánt + krížová podložka na paneli | ✅ | značky Ø4 × 2 chýbajú (nástroj) | 11 §5.2 |
| Tlmiace čočky | ✅ | | 11 §1.8 |
| TIP-ON | 🟡 | iba súpis, vŕtanie chýba | 11 §1.6 |
| Merivobox (NL, výšky N/M/K/E, diery, čelo) | ✅ | | 09 §3, 10 §4 |
| Iné výsuvy (Movento, Tandem, Antaro…) | ❌ | dielňa ich v zákazkách nemá | 10 §3.4 |
| Nožičky AXILO: počet ✅, značky Ø4 × 2 ❌ | 🟡 | REF: hrot (typ L) Ø4 × 2 na tabuli, 4 značky na nožičku (±32 okolo stredu 45 od konca dna, 108 od čela / 46 od zadu) | 11 §1.5 |
| Závesné kovanie horných (CAMAR) – súpis | ❌ | na CNC sa nevŕta | 09 §5.5 |

## 5. Katalóg a atribúty
| Oblasť | Stav | Pozn. | Zdroj |
|---|---|---|---|
| Katalógy, vlastný katalóg, uloženie skrinky | ✅ | | 11 §2 |
| Kuchyne_CASTEL (bežné spodné, horné, rohové, drez, umývačka) | ✅ | | 10, 11 §5 |
| BLENDA (lišta 18 × 100 a slepé čelo 78 = 18 + OLR 60, 3 kolíky 32/381/730) | ✅ | skrinka bez korpusu s `filler`, položka v Kuchyne_CASTEL; parita REF skr. 5: 6/6 dier | 11 §5.3 |
| DOSKA, KRYCI BOK, RURA, MIKROVLNKA, dvere, Sevroll… | ❌ | | 10 §8 |
| KRYCI BOK pri skrinke (REF skr. 13: 899 × 500 × 38) | ✅ | atribút `cover_side_left/right` (import z CV), rozmery a materiál zo `[cover_side]` (pracovná doska 38, od podlahy po vrch korpusu + 37, 28 pred čelom, 80 za korpus); samostatná položka KRYCI BOK 19 × 2464 × 600 z katalógu nie | 10 §8, 12 §6 |
| BLENDA, DOSKA, RURA, MIKROVLNKA, dvere, Sevroll… | ❌ | | 10 §8 |
| Editor sekcií (deliť otvor myšou) | 🟡 | predvoľby a stĺpce, bez kreslenia | 11 §3 |

## 5a. Kreslenie (ovládanie ako v CV, findings 06 §4, 11 §3, pozorovanie CV 2026-10-17)
| Oblasť | Stav | Pozn. | Zdroj |
|---|---|---|---|
| Vloženie z katalógu myšou (prichytenie k stene, dotyk so susedom), bez myši do stredu steny | ✅ | | 06 §4.3 |
| Polia skrinky: Clearance, End Position, Center Position, Dimensions + Lock Width, Position; „+50“ / „-50“ | ✅ | uhol (Angle) chýba | 06 §4.2, CV |
| Úchyty (stred = posun, hrany = šírka), zámok šírky | ✅ | otáčací úchyt chýba | 06 §4.2, 11 §4.2 |
| Pravý klik na skrinku: Kopírovať, Zmazať, Sekcia, Zamknúť šírku, Vlastnosti | 🟡 | Save, Edit Shape, Combine, Split, Detach Toe, Finish End, Replace chýbajú | CV |
| Vlastnosti skrinky (záložky General, Comment, Legs, Overrides, Parameters) | ✅ | Finishes nie (materiály v projekte) | CV |
| Editor sekcií: otvor 1A…, Face Size (rozmery, zámky, typ, pánt, úpravy), Split H/V, Lock/Equalize, Delete | ✅ | stĺpec sa ďalej vodorovne nedelí; rozmer otvoru = čelo (CV: otvor so škárami) | 11 §3, CV |
| Pravý klik do rohu: Extend Left/Right End (slepý roh s panelom) | ✅ | pri zásuvkách nie; Make Void, Miter, Corner (tvarovanie) chýbajú | 06 §4.4 |
| Výškové umiestnenie Placement (podlaha, sokel, pod/na pracovnú dosku, pod horné, pod/nad podhľad, strop, spodok/vrch vo výške, voľne) | ✅ | horné dielne Above Elevation 1518 v katalógu | 06 §4.3, 11 §4.1 |
| Snap to Corner: prisunúť do ľavého / pravého rohu | 🟡 | bez diagonálneho natočenia (skrinky nemajú uhol); kuchynské ROH dielne natočenie nepoužívajú | 06 §4.3, 11 §4.3 |
| Voľné umiestnenie mimo steny (ostrov), uhol | ❌ | | 06 §4.2 |

## 6. Výstupy
| Oblasť | Stav | Pozn. | Zdroj |
|---|---|---|---|
| CX100 `.xcs`, nesting tabule, CncRun | ✅ | verified = false | 08 |
| Štítky, výkresy PDF, rozpis CSV | ✅ | | |
| Súpis kovania | ✅ | v okne a `model.hardware`; do CSV/PDF ❌ | 11 §1.3 |
| Import `.cvj`: steny, skrinky | ✅ | | 07 |
| Import `.cvj`: čelá zo stromu otvorov, zásuvky (BOXH, NL), škáry, úpravy OL*, atribúty, umývačka iba s čelom | ✅ | REF: čelá do 0,5 mm ako CV okrem čela 130 (kontrola min. dielu stroja) | 09, 10, 11 §1, §3 |
| Zápis `.cvj` pre CV (`cc-out-cvj`, šablóna = zákazka CV dielne) | 🟡 | skrinky ako kostra (parametre + sekcie, CV diely prestaví), rozmer CV = vzor + zmena oproti importu, skrinka bez väzby na katalóg, doplnky bez PID (sokel) ostanú, delenie čiel z `fronts` (zásuvky, dvierka, strana pántov, Pair, stĺpce, BOXH/BOXD, OL*, EQ); REF sa zapíše s rovnakými sekciami ako originál; steny (X Z DX DY DZ AY + `_AI/_DL/_DR` a zadné plochy vzorcami CV, kontrola `walls::check`), okná/dvere/otvory ako kópie vzoru šablóny. Doplnky CV pri okne (parapet) idú s oknom. Čelá nezmenené od importu (aj zjednodušené pre náš generátor) ostanú ako v CV (`cv_fronts`), vynechané skrinky ostanú (`cv_skipped`). Príkaz `project.export_cvj`, Súbor → Uložiť do CABINET VISION (.cvj)…, `cabinetcraft export-cvj`. Overené vlastníkom v CV 2026-10-17: skrinky, sokel, okno s rámom a sklom, parapet. Steny: zmazaná stena sa vynechá aj s objektmi, nová je kópia steny šablóny (čísla, susedia, kódy plôch podľa geometrie); dvere/otvor bez vzoru = obálka z okna s TYPE 20/26. Chýba: oblúkové steny; náš `.cvj` sa dá znova importovať až po uložení v CV (kostry nemajú diely) | 13 §10 |
| Drezová skrinka (S_KUCH_DREZ) | ✅ | `top_style = "sink"`: predný vlys zvislý hore (lícuje s vrchom bokov), zadný „CHRBAT“ zvislý na dne, spoj s dnom D-K-D od 64 (`end_offset`) s konfirmátmi cez dno; korpus zásuvky o odsadenie dna vyššie. REF: všetky diely drezu okrem značiek Ø4 nožičiek | 12 §4 |
| Slepý roh (S_KUC_ROH_P/_L) | ✅ | pevný panel v krajnom stĺpci čiel: bok na jeho strane vystupuje k panelu (`[blind_corner] side_extend` 3, LU 391 = D − 4 + 3), panel na prednej hrane boku vzorom 3/3/3 (kolíky + K Ø5 × 13 do panelu), s prednou hranou dna cez medzeru 3 kolíkmi `panel_bottom_dowels` (32/96/350/414 od boku; pravidlo polôh [N]); import hĺbky bez vystupujúceho boku (392). REF: všetky diely rohu okrem značiek Ø4 × 2 (nožičky, slepý pánt) a hrúbky chrbta | 11 §5.2, 12 §1 |
| Editor skrinky (dvojklik na skrinku ako v CV) | ✅ | záložka Skrinka: samotná skrinka v 3D (výber dielu, čelá, rozloženie, spredu/izo/zhora), pohľad spredu so sekciami 1A… (klik = vlastnosti otvoru v ľavom paneli), diely skrinky (dvojklik = výkres dielu); otvorí dvojklik v pôdoryse, pohľade aj 3D, ponuka skrinky a tlačidlo v paneli | CV |
| Štítky dielov ako Label IT / +Label | ✅ | návrh 80 × 80 podľa CV (polia, písmo podľa poľa, náčrt v mierke s krúžkami hrán, Up/Down, symbol orientácie, Code 39), rozmer a poradie tlače `[labels]` v machine.toml, číslo dielu = poradie prerezania na tabuli; záložka Štítky (tabuľa, klik na diel = štítok, tlač tabule), `labels.sheets`/`labels.print`. Zvyšok tabule sa do čísla počíta ako v CV (`offcut_number`), priama tlač na tlačiareň (`printer`, voľba „tlačiť hneď“). Radenie podľa pásov píly nie | 14 |

## Postup
1. Referenčná zákazka REF2026: dataset dielov z CV (rozmery, materiál, olepenie, všetky
   diery a drážky) a import `.cvj` s čelami a atribútmi → porovnanie diel po diele.
2. Opraviť rozdiely zhora tabuľky (korpus, rad dier, škáry), každú opravu s testom proti CV.
