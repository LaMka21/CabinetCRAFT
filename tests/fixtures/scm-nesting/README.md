# Vzory SCM nesting (+ súvisiace CX100), anonymizované

Programy, ktoré CABINET VISION 2026 (S2M + AlphaCAM post) vygeneroval pre zákazku REF2026
(25. 9. 2026, beh 966 = NESTING, beh 968 = NESTING PD).

**Anonymizácia:** zo všetkých `.xcs` sú odstránené riadky komentárov `//...` (cesta k zákazke,
meno zákazníka, dátum). Ostatné bajty sú bez zmeny (ASCII, CRLF, bez CRLF na konci súboru).
Bitmapy štítkov (`*.bmp`) ani `CncRun*.txt` sem zámerne nie sú skopírované, lebo obsahujú
názov zákazky so zákazníkom. Rozbor formátov: `findings/08-nesting-scm.md`.

| Súbor | Obsah |
|---|---|
| `nest/9660301N.xcs` | tabuľa 2800 × 2070 × 18 (DTDL W960): 272 vŕtaní (Ø8, Ø5, Ø4 hrot), 2 onion-skin prechody malých dielov (hĺbka 16), 1 spojený obrys všetkých 18 dielov (hĺbka 18,2), park |
| `nest/9660401N.xcs` | tabuľa 2800 × 2070 × 16: bez vŕtania, 4 onion-skin (hĺbka 14), spojený obrys (16,2), odrezanie zvyšku tabule `SquareOffNest` (os nástroja, y = 531,2) |
| `nest/9680601N.xcs` | pracovná doska 4100 × 650 × 38: iba výrez pre drez 760 × 480, R15, fréza E012 Ø20, hĺbka 38,2 s nábehom aj výbehom |
| `cx100/*.xcs` | 11 programov CX100 pre diely z tabule 9660301N (hrany Ø8 × 26; `966f0020` aj vŕtanie plochy F, `966b0056` drážka pílou) |
| `labels-966.plp` | pozície štítkov behu 966 (palce, stred dielu, súradnice tabule), bez osobných údajov |
| `layout.csv` | **odvodené** (nie výstup CV): rekonštrukcia obdĺžnikov dielov na tabuliach 9660301N a 9660401N z obrysovej dráhy + `.plp` + `.ard`; stĺpce: tabuľa, diel, x/y min/max, rozmery, onion skin, počet vŕtaní na nestingu, program CX100 a jeho L × W, text `Up/Down` zo štítku |

Konvencie, ktoré z nich vyplývajú (podrobne vo findings 08):
- súradnice tabule: počiatok vľavo dole, X po dĺžke 2800, Y po šírke 2070; okraj 10 mm, medzera medzi dielmi 14,2 mm,
- obrysy majú geometriu hotového dielu, `CreateRoughFinish(...,"E002","-1",1,12,...)` = korekcia vľavo, Ø12, súsledné frézovanie,
- hĺbka prerezania = hrúbka + 0,2; onion skin = hrúbka − 2.
