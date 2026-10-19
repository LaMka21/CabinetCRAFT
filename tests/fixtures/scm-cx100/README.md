# Vzory formátu SCM Maestro .xcs pre CX100

Príkazy z programov, ktoré vygeneroval CABINET VISION pre zákazku REF2026
(NESTING\CX100, 25. 9. 2026). Komentáre s názvom zákazky a zákazníka sú odstránené.

Použitie: `crates/cc-out-xcs/tests/conformance.rs` overuje, že výstup CabinetCRAFT
používa iba funkcie, počty argumentov a roviny, ktoré sa vyskytujú v týchto programoch.

| Súbor | Obsah |
|---|---|
| 966b0047.xcs | bok 663 × 351 × 19: vŕtanie Top, Left, Right |
| 966b0051.xcs | dno 485 × 351 × 19: drážka 4 prechodmi pílky |
| 966f0049.xcs | chrbát 663 × 447 × 19: vŕtanie Front, Back, Left, Right |
| 966b0039.xcs | vrch 626.5 × 330 × 19: jedna drážka pílkou |
| 966f0040.xcs | dno hornej skrinky 540.5 × 330 × 18: dvojice kolíkov Left/Right, drážka zadnej steny |
| 966f0020.xcs | dno spodnej skrinky 564 × 510 × 18: tri dvojice kolíkov |
| 966f0004.xcs | vlys vrchu 564 × 128 × 18: dva kolíky 18 / 110 |

`crates/cc-out-xcs/tests/dielna.rs` porovnáva s týmito programmi výstup štandardu dielne.
