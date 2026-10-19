//! Kategórie množného čísla podľa jazyka (zjednodušené pravidlá CLDR pre celé čísla
//! a desatinné čísla). Neznámy jazyk dostane pravidlo „1 = one, inak other“.

/// Kategória množného čísla, ako ju píše prekladateľ v súbore (`one`, `few`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Zero,
        Category::One,
        Category::Two,
        Category::Few,
        Category::Many,
        Category::Other,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Category::Zero => "zero",
            Category::One => "one",
            Category::Two => "two",
            Category::Few => "few",
            Category::Many => "many",
            Category::Other => "other",
        }
    }

    pub fn from_name(name: &str) -> Option<Category> {
        Category::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// Pravidlá, ktoré pozná [`category`]. Prekladateľ ich môže v `[meta] plural` zvoliť
/// pre jazyk, ktorý tu nie je (napr. `plural = "ru"` pre podobný slovanský jazyk).
pub const RULES: &[&str] = &[
    "en", "sk", "cs", "pl", "ru", "uk", "be", "hr", "sr", "bs", "sl", "lt", "lv", "ro", "fr", "pt",
    "ar", "ja", "zh", "ko", "vi", "th", "id",
];

/// Kategória pre číslo `n` v jazyku `lang` (`sk`, `pt-BR`, …).
pub fn category(lang: &str, n: f64) -> Category {
    use Category::*;
    if !n.is_finite() {
        return Other;
    }
    let primary = lang
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let abs = n.abs();
    let int = abs.fract() == 0.0;
    // celá časť; čísla mimo rozsahu u64 sú „many/other“ v každom jazyku
    let i = if abs < 1e18 {
        abs.trunc() as u64
    } else {
        u64::MAX
    };
    let m10 = i % 10;
    let m100 = i % 100;
    match primary.as_str() {
        "sk" | "cs" => match (int, i) {
            (true, 1) => One,
            (true, 2..=4) => Few,
            (false, _) => Many,
            _ => Other,
        },
        "pl" => {
            if !int {
                Other
            } else if i == 1 {
                One
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                Few
            } else {
                Many
            }
        }
        "ru" | "uk" | "be" => {
            if !int {
                Other
            } else if m10 == 1 && m100 != 11 {
                One
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                Few
            } else {
                Many
            }
        }
        "hr" | "sr" | "bs" => {
            if int && m10 == 1 && m100 != 11 {
                One
            } else if int && (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                Few
            } else {
                Other
            }
        }
        "sl" => match (int, m100) {
            (true, 1) => One,
            (true, 2) => Two,
            (true, 3..=4) => Few,
            _ => Other,
        },
        "lt" => {
            if !int {
                Many
            } else if m10 == 1 && !(11..=19).contains(&m100) {
                One
            } else if m10 >= 2 && !(11..=19).contains(&m100) {
                Few
            } else {
                Other
            }
        }
        "lv" => {
            if int && (m10 == 0 || (11..=19).contains(&m100)) {
                Zero
            } else if int && m10 == 1 && m100 != 11 {
                One
            } else {
                Other
            }
        }
        "ro" => {
            if int && i == 1 {
                One
            } else if !int || i == 0 || (2..=19).contains(&m100) {
                Few
            } else {
                Other
            }
        }
        "fr" | "pt" => {
            if i <= 1 {
                One
            } else {
                Other
            }
        }
        "ar" => {
            if !int {
                Other
            } else {
                match (i, m100) {
                    (0, _) => Zero,
                    (1, _) => One,
                    (2, _) => Two,
                    (_, 3..=10) => Few,
                    (_, 11..=99) => Many,
                    _ => Other,
                }
            }
        }
        "ja" | "zh" | "ko" | "vi" | "th" | "id" | "ms" => Other,
        _ => {
            if int && i == 1 {
                One
            } else {
                Other
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Category::*;
    use super::*;

    #[test]
    fn slovak_and_english() {
        assert_eq!(category("sk", 1.0), One);
        assert_eq!(category("sk", 3.0), Few);
        assert_eq!(category("sk", 5.0), Other);
        assert_eq!(category("sk", 0.0), Other);
        assert_eq!(category("sk", 1.5), Many);
        assert_eq!(category("en", 1.0), One);
        assert_eq!(category("en", 0.0), Other);
        assert_eq!(category("en-GB", 2.0), Other);
    }

    #[test]
    fn slavic_and_others() {
        assert_eq!(category("pl", 22.0), Few);
        assert_eq!(category("pl", 12.0), Many);
        assert_eq!(category("ru", 21.0), One);
        assert_eq!(category("ru", 11.0), Many);
        assert_eq!(category("fr", 0.0), One);
        assert_eq!(category("pt-BR", 2.0), Other);
        assert_eq!(category("ar", 103.0), Few);
        assert_eq!(category("ja", 1.0), Other);
        assert_eq!(category("xx", 1.0), One);
        assert_eq!(category("sk", f64::NAN), Other);
        assert_eq!(category("ru", -1e300), Many);
    }
}
