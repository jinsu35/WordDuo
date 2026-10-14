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

`View › Bilingual › Bilingual View` (on by default) lays out a *mirror* of the document in which each paired
paragraph shows its translation, keeping the source paragraph's formatting
(`crates/engine/src/bilingual.rs`). The shorter language of each pair is padded
(`LayoutOptions::pad_after`) so both documents paginate alike, so every paragraph and its
translation start at the same place on the same page. The canvas crossfades the two page images.

You always type into the language you are **working in**. Switching it swaps every paragraph
with its translation, in the document and in the undo history, so the language you work in is
the one that is edited, printed, exported and saved as visible text. The other one stays hidden
beside it. Switching is not an undo step.

| Gesture | Effect |
|---|---|
| **Tap ⌥ Option** alone (Ctrl on Windows and Linux) | Switch the language you work in |
| **Hold ⌥ Option** alone, ¼ s or more (Ctrl elsewhere) | Peek at the other language while held. ⌥⌫, ⌥← and other Option chords never peek |
| **Click while peeking** | Work in the language peeked at, with the insertion point where you clicked |
| **Twist two fingers** on the trackpad, like a dial (about 35° end to end) | Blend from one language to the other; past the middle, the language you work in switches too |
| Badge above the page | The page's language: filled while you work in it (Korean blue, English amber), outlined while you peek at the other one; it grows for a moment when the language switches |
| Slider at the bottom of the page (the language you work in is in bold) | Same as twisting, by mouse |
| Pinch, ⌘ / Ctrl + scroll | Zoom the page, as always |

Pairs are made paragraph by paragraph (Enter starts the next one). A paragraph you write in
one language is in that language only: the other language's page shows an empty paragraph in
its place, with the text you wrote greyed as a hint until you write its translation there. The
hint is only shown, never saved. Splitting a paragraph keeps its translation with the piece
that kept most of its text.

The view is on by default. A document without translations looks as it would without it, and
the switch key works there too: the first tap gives every paragraph an empty translation and
switches to it, to write the other language (a new document, written in one language so far).

## Commands

| Id | What it does |
|---|---|
| `bilingual.view` | Bilingual view on/off |
| `bilingual.mix` `{"value": 0–1}` | Blend (0: the text the document opened with, 1: its translation) |
| `bilingual.switch` `{"value"?: bool, "keepBlend"?: bool}` | Work in the other language (see above) |
| `bilingual.edit` | Show translations under their paragraphs, to write and edit them |
| `bilingual.translate` `{"text"?, "lang"?}` | Add or set the translation of the paragraph at the caret. Without `text` it opens it for editing. The default language is Korean for English text and English for Korean |
| `bilingual.pair` `{"by"?: "language" \| "alternate"}` | Turn an interleaved document (EN, KO, EN, KO…) into pairs. Empty paragraphs between a pair are removed |
| `bilingual.swap` | Swap every paragraph with its translation, as an undoable edit |
| `bilingual.pairs` | List the pairs (for agents, the CLI and MCP) |

For example:
`wordcraft-cli run --cmd 'bilingual.pair' --cmd 'bilingual.view' --save out.docx in.docx`.

## Limits (for now)

- Only top-level body paragraphs pair; paragraphs in tables, headers, footers and notes don't.
- While the two are blended, typing goes to the language you work in (the bold one).
- Saving keeps the language you were working in as the visible text: Word shows that one.
- Pagination matches line for line. A paragraph that splits across pages can split at a
  different line in each language when their line heights differ.
