//! Spoločné pomôcky pre výstupy pre stroje (`cc-out-mpr`, `cc-out-xcs`).

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

/// Číslo s max. `decimals` desatinnými miestami, bez núl na konci a bez „-0“.
pub fn fmt_num(v: f64, decimals: u32) -> String {
    let s = format!("{v:.*}", decimals as usize);
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s
    };
    if s == "-0" {
        "0".to_owned()
    } else {
        s
    }
}

/// Dosadí `{premenná}`. Neznáme premenné ostanú bez zmeny (kontroluje ich cc-config).
pub fn substitute(template: &str, vars: &[(&str, String)]) -> String {
    let mut out = template.to_owned();
    for (k, v) in vars {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// Bezpečný názov súboru z ID dielu.
pub fn file_stem(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// ASCII: slovenská/česká diakritika → základné písmeno, ostatné ne-ASCII → `?`.
pub fn to_ascii(text: &str) -> Vec<u8> {
    const MAP: &[(char, char)] = &[
        ('á', 'a'),
        ('ä', 'a'),
        ('č', 'c'),
        ('ď', 'd'),
        ('é', 'e'),
        ('ě', 'e'),
        ('í', 'i'),
        ('ĺ', 'l'),
        ('ľ', 'l'),
        ('ň', 'n'),
        ('ó', 'o'),
        ('ô', 'o'),
        ('ö', 'o'),
        ('ŕ', 'r'),
        ('ř', 'r'),
        ('š', 's'),
        ('ť', 't'),
        ('ú', 'u'),
        ('ů', 'u'),
        ('ü', 'u'),
        ('ý', 'y'),
        ('ž', 'z'),
        ('Á', 'A'),
        ('Ä', 'A'),
        ('Č', 'C'),
        ('Ď', 'D'),
        ('É', 'E'),
        ('Ě', 'E'),
        ('Í', 'I'),
        ('Ĺ', 'L'),
        ('Ľ', 'L'),
        ('Ň', 'N'),
        ('Ó', 'O'),
        ('Ô', 'O'),
        ('Ö', 'O'),
        ('Ŕ', 'R'),
        ('Ř', 'R'),
        ('Š', 'S'),
        ('Ť', 'T'),
        ('Ú', 'U'),
        ('Ů', 'U'),
        ('Ü', 'U'),
        ('Ý', 'Y'),
        ('Ž', 'Z'),
        ('×', 'x'),
        ('–', '-'),
        ('—', '-'),
        ('„', '\''),
        ('“', '\''),
    ];
    text.chars()
        .map(|c| {
            if c.is_ascii() {
                c as u8
            } else {
                MAP.iter()
                    .find(|(from, _)| *from == c)
                    .map_or(b'?', |(_, to)| *to as u8)
            }
        })
        .collect()
}

/// Spojí riadky zvoleným koncom riadku a ukončí aj posledný riadok.
pub fn join_lines(lines: &[String], crlf: bool) -> String {
    let eol = if crlf { "\r\n" } else { "\n" };
    let mut text = lines.join(eol);
    text.push_str(eol);
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_format() {
        assert_eq!(fmt_num(720.0, 3), "720");
        assert_eq!(fmt_num(151.375, 3), "151.375");
        assert_eq!(fmt_num(545.75, 3), "545.75");
        assert_eq!(fmt_num(1.0 / 3.0, 3), "0.333");
        assert_eq!(fmt_num(492.666_666, 4), "492.6667");
        assert_eq!(fmt_num(-0.0001, 3), "0");
        assert_eq!(fmt_num(25.4, 0), "25");
    }

    #[test]
    fn substitution() {
        let vars = [("x", "10".to_owned()), ("depth", "12.5".to_owned())];
        assert_eq!(substitute("{x}", &vars), "10");
        assert_eq!(substitute("-{depth}", &vars), "-12.5");
        assert_eq!(substitute("LS", &vars), "LS");
    }

    #[test]
    fn ascii_output() {
        assert_eq!(
            to_ascii("Kuchyňa – Šťastná × 2"),
            b"Kuchyna - Stastna x 2".to_vec()
        );
        assert_eq!(to_ascii("€"), b"?".to_vec());
    }

    #[test]
    fn safe_file_names() {
        assert_eq!(file_stem("S1-SIDE-L"), "S1-SIDE-L");
        assert_eq!(file_stem("A/B C"), "A_B_C");
    }

    #[test]
    fn lines() {
        assert_eq!(join_lines(&["a".into(), "b".into()], true), "a\r\nb\r\n");
        assert_eq!(join_lines(&["a".into()], false), "a\n");
    }
}
