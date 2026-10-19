//! Čiarový kód Code 39 (znaky 0–9, A–Z, `- . $ / + %` a medzera, štart/stop `*`).

/// Vzor znaku: 9 prvkov (čiara, medzera, čiara, …), `true` = široký.
fn pattern(c: char) -> Option<[bool; 9]> {
    let p: &str = match c {
        '0' => "000110100",
        '1' => "100100001",
        '2' => "001100001",
        '3' => "101100000",
        '4' => "000110001",
        '5' => "100110000",
        '6' => "001110000",
        '7' => "000100101",
        '8' => "100100100",
        '9' => "001100100",
        'A' => "100001001",
        'B' => "001001001",
        'C' => "101001000",
        'D' => "000011001",
        'E' => "100011000",
        'F' => "001011000",
        'G' => "000001101",
        'H' => "100001100",
        'I' => "001001100",
        'J' => "000011100",
        'K' => "100000011",
        'L' => "001000011",
        'M' => "101000010",
        'N' => "000010011",
        'O' => "100010010",
        'P' => "001010010",
        'Q' => "000000111",
        'R' => "100000110",
        'S' => "001000110",
        'T' => "000010110",
        'U' => "110000001",
        'V' => "011000001",
        'W' => "111000000",
        'X' => "010010001",
        'Y' => "110010000",
        'Z' => "011010000",
        '-' => "010000101",
        '.' => "110000100",
        ' ' => "011000100",
        '*' => "010010100",
        '$' => "010101000",
        '/' => "010100010",
        '+' => "010001010",
        '%' => "000101010",
        _ => return None,
    };
    let mut out = [false; 9];
    for (slot, ch) in out.iter_mut().zip(p.chars()) {
        *slot = ch == '1';
    }
    Some(out)
}

/// Text, ktorý sa zakóduje: veľké písmená, `_` → `-`. `None`, ak obsahuje nepodporovaný znak.
pub fn normalize(text: &str) -> Option<String> {
    let s: String = text
        .chars()
        .map(|c| {
            if c == '_' {
                '-'
            } else {
                c.to_ascii_uppercase()
            }
        })
        .collect();
    (!s.is_empty() && s.chars().all(|c| c != '*' && pattern(c).is_some())).then_some(s)
}

/// Čiary kódu ako (začiatok, šírka) v násobkoch úzkeho modulu, vrátane štart/stop `*`.
/// Pomer širokého a úzkeho prvku 2,5; medzi znakmi medzera jedného modulu.
pub fn bars(text: &str) -> Option<(Vec<(f64, f64)>, f64)> {
    const WIDE: f64 = 2.5;
    let s = normalize(text)?;
    let mut x = 0.0;
    let mut out = Vec::new();
    let all: Vec<char> = std::iter::once('*')
        .chain(s.chars())
        .chain(std::iter::once('*'))
        .collect();
    for (i, c) in all.iter().enumerate() {
        let p = pattern(*c)?;
        for (k, wide) in p.iter().enumerate() {
            let w = if *wide { WIDE } else { 1.0 };
            if k % 2 == 0 {
                out.push((x, w));
            }
            x += w;
        }
        if i + 1 < all.len() {
            x += 1.0;
        }
    }
    Some((out, x))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pattern_has_three_wide_elements() {
        for c in "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. *$/+%".chars() {
            let p = pattern(c).unwrap();
            assert_eq!(p.iter().filter(|w| **w).count(), 3, "{c}");
        }
    }

    #[test]
    fn bars_round_trip_by_widths() {
        let (b, total) = bars("sp1-bottom").unwrap();
        // 12 znakov × 5 čiar
        assert_eq!(b.len(), 12 * 5);
        // znak = 6 úzkych + 3 široké = 13,5 modulu, 11 medzier medzi znakmi
        assert!((total - (12.0 * 13.5 + 11.0)).abs() < 1e-9);
        assert_eq!(normalize("HO1_ASHELF.1").as_deref(), Some("HO1-ASHELF.1"));
        assert!(bars("ľ").is_none());
    }
}
