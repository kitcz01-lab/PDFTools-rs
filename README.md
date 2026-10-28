# PDFTools (Rust Edition)

Rychlá, moderní a paměťově nenáročná desktopová aplikace pro práci s PDF dokumenty napsaná v jazyce **Rust** s nativním grafickým rozhraním **Slint GUI**.

Aplikace funguje jako plně samostatný nativní binární soubor bez jakékoliv závislosti na běhovém prostředí Java.

---

## Hlavní funkce

- 📄 **Slučování dokumentů (Merge)**:
  - Spojení více PDF souborů do jednoho výsledného dokumentu.
  - Podpora libovolného přeuspořádání souborů (Nahoru / Dolů).
  - Korektní zachování a materializace děděných vlastností stránek (MediaBox, CropBox, rotace stránek).
- ✂️ **Rozdělování dokumentů (Split)**:
  - Rozdělení PDF po zadaném počtu stránek do samostatných částí.
  - Automatické vyčištění nepoužitých objektů (`prune_objects`).
- 🗜️ **Komprese dokumentů (Compress)**:
  - Rekomprese vložených bitmapových JPEG a PNG obrázků s nastavitelnou kvalitou.
  - Bezztrátová komprese objektových proudů PDF.
  - Výpočet a zobrazení reálně ušetřené velikosti.
- ⚙️ **Ukládání nastavení**:
  - Trvalé uložení voleb (kvalita, počet stránek, automatické otevírání výsledků) v souladu se standardem XDG (`~/.config/pdftools/settings.json`).

---

## Použité technologie

- **Jazyk**: [Rust 2021](https://www.rust-lang.org/)
- **GUI**: [Slint](https://slint.dev/) (nativní, Wayland/X11 a Windows podpora)
- **Manipulace s PDF**: [`lopdf`](https://crates.io/crates/lopdf)
- **Zpracování obrazu**: [`image`](https://crates.io/crates/image)
- **Systémové dialogy**: [`rfd`](https://crates.io/crates/rfd)

---

## Sestavení a spuštění

### Požadavky
- Nainstalovaný Rust toolchain (`rustc` a `cargo`).

### Spuštění ve vývojovém režimu
```bash
cargo run
```

### Sestavení optimalizovaného release balíčku
```bash
cargo build --release
```
Výsledný binární soubor najdete v `target/release/pdftools`.
