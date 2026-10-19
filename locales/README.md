# Translating CabinetCRAFT

Every text that CabinetCRAFT shows (ribbon, panels, dialogs, status bar, command names,
PDF drawings and labels) lives in this folder, one file per language:

| File | Language | |
|---|---|---|
| `en.toml` | English | **reference file** – copy this one |
| `sk.toml` | Slovenčina | default language |

Both are built into the program. Any other `<code>.toml` file is loaded at run time, so you
do **not** need Rust or a build to add a language.

## Add a language

1. **Copy** `en.toml` and name the copy after the language's
   [ISO 639-1 code](https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes): `de.toml`,
   `fr.toml`, `pl.toml`. For a regional variant add the region: `pt-BR.toml`.
2. **Set the name** shown in the language list, in the language itself:
   ```toml
   [meta]
   name = "Deutsch"
   ```
3. **Translate** the text in quotes. Never change the keys (left of `=`) or the section
   names in `[brackets]`.
   ```toml
   [ribbon]
   return = "Zurück"            # was: return = "Return"
   ```
4. **Try it without building**: put the file into one of these folders and pick the
   language in the ribbon: **Tools → Language**. The list refreshes when you open it,
   no restart is needed; choosing the language again reloads your edited file.
   - `%APPDATA%\CabinetCRAFT\locales\` (the **Languages Folder** button opens it), or
   - a `locales` folder next to `cabinetcraft-app.exe`.

   If the file has a syntax error, the language list shows the error with its line number.
5. **Check** the file (optional): `cabinetcraft check-locale de.toml` lists missing keys,
   keys that do not exist in English and texts whose `{parameters}` differ.
   With a Rust toolchain, `cargo xtask i18n` checks every file in this folder.
6. **Send a pull request** that adds `locales/<code>.toml`. You don't have to translate
   everything at once – a missing or empty text falls back to English.

You can also fix the built-in languages the same way: a `sk.toml` or `en.toml` in a
`locales` folder overrides single texts of the built-in file.

## File format

The files are [TOML](https://toml.io): `key = "text"`, grouped by sections that follow
the parts of the window (`[ribbon]`, `[side_panel.cabinet]`, `[pdf.drawings]`, …).
Save the file as UTF-8.

- **Quotes and backslashes**: inside `"…"` write `\"` for a quote and `\\` for a
  backslash, or use single quotes `'C:\folder'` (no escapes).
- **Line breaks**: `\n` inside the text.
- **Parameters** in braces are replaced by the program – keep them, you may move them:
  ```toml
  saved = "Project {name} saved"          # en
  saved = "Projekt {name} gespeichert"    # de
  ```
  `{{` and `}}` produce literal braces.
- **Plural forms**: a text that depends on a number lists its forms:
  ```toml
  parts = { one = "{count} part", other = "{count} parts" }                        # en
  parts = { one = "{count} díl", few = "{count} díly", other = "{count} dílů" }     # cs
  ```
  The forms are the [CLDR plural categories](https://cldr.unicode.org/index/cldr-spec/plural-rules)
  `zero`, `one`, `two`, `few`, `many`, `other`; `other` is required, a missing one falls back to
  `other`. The program knows the rules for Slovak, Czech, Polish, Russian, Ukrainian,
  Belarusian, Croatian, Serbian, Bosnian, Slovenian, Lithuanian, Latvian, Romanian, French,
  Portuguese, Arabic, languages without plurals (Japanese, Chinese, Korean, Vietnamese, Thai,
  Indonesian) and "one / other" for everything else. If your language needs the rule of
  another one, add it to `[meta]`: `plural = "ru"`.
- **Comments** start with `#`.

## Words

English follows the terms carpenters know from cabinet design software (Walls, Objects,
Elevation, Assembly, Return, Section, Face, Auto Fill, …). Use the words your
workshop uses – short, so they fit the ribbon buttons.

## What is not translated

Machine programs (`.xcs`, `.mpr`, `RUN.bat`, nesting), CSV cut lists, `model.json`,
command ids and JSON parameters of the control channel, and the names of parts, materials
and cabinets that come from project and standard files.

The window uses the Segoe UI font (Latin, Greek, Cyrillic). Scripts it does not cover,
such as Chinese or Japanese, would need an extra font – open an issue if you want to
translate into such a language.
