# Bilingual view (for translators and interpreters)

Status: first cut (fork feature, not in upstream WordCraft).

Each paragraph can carry its translation, and the page can show either language **in the same
place**: flip between them, or blend one over the other.

## How a translation is stored

A translation is the paragraph right after its source paragraph, set in the **Translation**
paragraph style. That style is hidden text (`w:vanish`) in blue, so:

- the document is still one ordinary `.docx`; Word opens it and shows the source text, with the
  translations as hidden text (File › Options › Display › Hidden text shows them);
- a paragraph hidden whole (text and mark) takes no room on the page, as in Word
  (`crates/layout/src/lib.rs`, `hidden_whole`).

## The view

`View › Bilingual › Bilingual View` lays out a *mirror* of the document in which each paired
paragraph shows its translation, keeping the source paragraph's formatting
(`crates/engine/src/bilingual.rs`). The shorter language of each pair is padded
(`LayoutOptions::pad_after`) so both documents paginate alike, so every paragraph and its
translation start at the same place on the same page. The canvas crossfades the two page images.

| Gesture | Effect |
|---|---|
| Hold **⌥ Option** (Alt) by itself | Show the other language while held |
| **Twist two fingers** on the trackpad (clockwise / anticlockwise), like a dial | Blend more / less of the translation |
| Slider at the bottom of the page | Same blend, by mouse |
| Pinch, ⌘ / Ctrl + scroll | Zoom the page, as always |

## Commands

| Id | What it does |
|---|---|
| `bilingual.view` | Bilingual view on/off |
| `bilingual.mix` `{"value": 0–1}` | Blend (0 text, 1 translation) |
| `bilingual.edit` | Show translations under their paragraphs, to write and edit them |
| `bilingual.translate` `{"text"?, "lang"?}` | Add or set the translation of the paragraph at the caret. Without `text` it opens it for editing. The default language is Korean for English text and English for Korean |
| `bilingual.pair` `{"by"?: "language" \| "alternate"}` | Turn an interleaved document (EN, KO, EN, KO…) into pairs. Empty paragraphs between a pair are removed |
| `bilingual.swap` | Swap every paragraph with its translation |
| `bilingual.pairs` | List the pairs (for agents, the CLI and MCP) |

For example:
`wordcraft-cli run --cmd 'bilingual.pair' --cmd 'bilingual.view' --save out.docx in.docx`.

## Limits (for now)

- Only top-level body paragraphs pair; paragraphs in tables, headers, footers and notes don't.
- The blended or flipped view is for reading. Editing types into the source text; edit
  translations with Edit Translations.
- Pagination matches line for line. A paragraph that splits across pages can split at a
  different line in each language when their line heights differ.
- Holding Alt on Windows and Linux doesn't open KeyTips while the view is on.
