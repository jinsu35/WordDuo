//! Bilingual documents, for translators and interpreters (Korean ↔ English and any other pair).
//!
//! A paragraph carries its translation in the paragraph right after it, set in the hidden
//! [`STYLE`] paragraph style. The translation travels with the document (in .docx it is a hidden
//! paragraph Word keeps and can show), prints nowhere and takes no room on the page.
//!
//! The bilingual view lays out a second, *mirror* document in which every paired paragraph shows
//! its translation instead (keeping the source paragraph's formatting), and pads the shorter
//! language of each pair ([`wordcraft_layout::LayoutOptions::pad_after`]) so both documents
//! paginate alike: each paragraph and its translation then sit in the same place on the same
//! page, and the editor can flip between them or blend one over the other.

use std::collections::HashMap;
use std::sync::Arc;

use wordcraft_doc::para::{Paragraph, next_rev};
use wordcraft_doc::props::{CharProps, Rgb, TextColor};
use wordcraft_doc::styles::{Style, StyleKind};
use wordcraft_doc::{Block, Document, Path, Pos, StoryRef};
use wordcraft_layout::{DocLayout, LayoutCache, LayoutOptions, Placed};

/// The paragraph style that marks a translation.
pub const STYLE: &str = "Translation";

/// Add the [`STYLE`] style to `doc` when it hasn't one: hidden text, in a colour that tells the
/// translation apart while it is shown for editing.
pub fn ensure_style(doc: &mut Document) {
    if doc.styles.get(STYLE).is_none() {
        doc.styles.styles.push(Style {
            id: STYLE.into(),
            name: "Translation".into(),
            kind: StyleKind::Paragraph,
            based_on: Some("Normal".into()),
            next: Some("Normal".into()),
            chr: CharProps { hidden: Some(true), color: Some(TextColor::Rgb(Rgb(0x1F, 0x5F, 0x8B))), ..Default::default() },
            priority: Some(99),
            ..Default::default()
        });
    }
}

/// Whether `p` is a translation (set in the [`STYLE`] style).
pub fn is_translation(p: &Paragraph) -> bool {
    p.props.style.as_deref() == Some(STYLE)
}

fn body_para(doc: &Document, block: usize) -> Option<&Paragraph> {
    doc.body.get(block)?.as_para()
}

/// The translation of the body paragraph at `block`: the translation paragraph right after it.
pub fn translation_of(doc: &Document, block: usize) -> Option<usize> {
    let p = body_para(doc, block)?;
    let next = block.checked_add(1)?;
    (!is_translation(p) && body_para(doc, next).is_some_and(is_translation)).then_some(next)
}

/// The source paragraph of the translation at `block`.
pub fn source_of(doc: &Document, block: usize) -> Option<usize> {
    let prev = block.checked_sub(1)?;
    (body_para(doc, block).is_some_and(is_translation) && translation_of(doc, prev) == Some(block)).then_some(prev)
}

/// Every (source, translation) pair of top-level body paragraphs, by block index, in order.
pub fn pairs(doc: &Document) -> Vec<(usize, usize)> {
    (0..doc.body.len()).filter_map(|b| translation_of(doc, b).map(|t| (b, t))).collect()
}

/// Whether `text` is mostly Hangul (Korean): more Hangul letters than Latin ones.
pub fn is_korean(text: &str) -> bool {
    let (mut hangul, mut latin) = (0usize, 0usize);
    for c in text.chars() {
        match c {
            '\u{AC00}'..='\u{D7A3}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' => hangul += 1,
            c if c.is_ascii_alphabetic() => latin += 1,
            _ => {}
        }
    }
    hangul > latin
}

/// The language tag a translation of `source` is written in: English for Korean, else Korean.
pub fn translation_lang(source: &str) -> &'static str {
    if is_korean(source) { "en-US" } else { "ko-KR" }
}

/// `source`'s place filled with `tr`'s text: the text, runs and inline objects of the
/// translation, shown (not hidden), with the source paragraph's formatting. Text with no colour
/// of its own takes the translation style's (`tint`), so blended over the source it stands apart.
fn shown_translation(source: &Paragraph, tr: &Paragraph, rev: u64, tint: Option<TextColor>) -> Paragraph {
    let mut p = source.clone();
    p.text = tr.text.clone();
    p.objects = tr.objects.clone();
    p.runs = tr.runs.clone();
    for r in &mut p.runs {
        r.props.hidden = None;
        if r.props.color.is_none() {
            r.props.color = tint;
        }
    }
    p.mark.hidden = None;
    p.rev = rev;
    p
}

/// Swap every paragraph with its translation: the translation becomes the text and the text
/// the translation. Each keeps its own place's paragraph formatting. Returns the pairs swapped.
pub fn swap_pairs(doc: &mut Document) -> usize {
    let all = pairs(doc);
    for (a, b) in &all {
        let (Some(pa), Some(pb)) = (body_para(doc, *a).cloned(), body_para(doc, *b).cloned()) else {
            continue;
        };
        if let Ok(x) = doc.para_mut(StoryRef::Body, &Path::top(*a)) {
            (x.text, x.runs, x.objects) = (pb.text, pb.runs, pb.objects);
            for r in &mut x.runs {
                r.props.hidden = None;
            }
            x.touch();
        }
        if let Ok(y) = doc.para_mut(StoryRef::Body, &Path::top(*b)) {
            (y.text, y.runs, y.objects) = (pa.text, pa.runs, pa.objects);
            y.touch();
        }
    }
    all.len()
}

/// Characters `a` and `b` share at their start or, if more, at their end.
fn shared_chars(a: &str, b: &str) -> usize {
    let prefix = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    let suffix = a.chars().rev().zip(b.chars().rev()).take_while(|(x, y)| x == y).count();
    prefix.max(suffix)
}

/// After an edit (`before` → `doc`) that split a paragraph with a translation (Enter, a paste of
/// several paragraphs), keep the translation with the paragraph it translates: the piece that
/// kept most of its text (the first piece, when even; the second, after Enter at its start). The
/// new paragraphs then come after the translation instead of taking it over. Top-level
/// selection paths follow the paragraphs that move. Returns whether anything moved.
pub fn keep_pairs(before: &Document, doc: &mut Document, sel: &mut crate::Selection) -> bool {
    let mut moved = false;
    for (s, t) in pairs(before) {
        let (Some(src), Some(tr)) = (before.body.get(s), before.body.get(t)) else { continue };
        let Some(source_text) = src.as_para().map(|p| p.text.clone()) else { continue };
        // The translation, untouched by the edit.
        let Some(j) = doc.body.iter().position(|b| Arc::ptr_eq(b, tr)) else { continue };
        // The nearest block before the source the edit left alone, in both documents.
        let anchor = (0..s).rev().find_map(|a| {
            let blk = before.body.get(a)?;
            doc.body.iter().position(|b| Arc::ptr_eq(b, blk)).map(|a2| (a, a2))
        });
        let (len_before, len_after) = match anchor {
            Some((a, a2)) => (t.saturating_sub(a + 1), j.saturating_sub(a2 + 1)),
            None => (t, j),
        };
        let Some(growth) = len_after.checked_sub(len_before).filter(|g| *g > 0) else { continue };
        let Some(first) = j.checked_sub(growth + 1) else { continue };
        // The pieces the source was split into: paragraphs, none of them a translation.
        let pieces: Option<Vec<(usize, usize)>> = (first..j)
            .map(|i| doc.body.get(i).and_then(|b| b.as_para()).filter(|p| !is_translation(p)).map(|p| (i, shared_chars(&p.text, &source_text))))
            .collect();
        let Some(pieces) = pieces else { continue };
        let Some(best) = pieces.iter().fold(None::<(usize, usize)>, |acc, &(i, sc)| match acc {
            Some((_, b)) if b >= sc => acc,
            _ => Some((i, sc)),
        }) else {
            continue;
        };
        let to = best.0 + 1;
        if to == j {
            continue;
        }
        let blk = doc.body.remove(j);
        doc.body.insert(to, blk);
        // Paragraphs between the piece and the translation's old place move down one.
        for p in [&mut sel.anchor, &mut sel.focus] {
            if p.story == StoryRef::Body
                && let Some(i) = p.path.0.first_mut()
                && (to as u32..j as u32).contains(i)
            {
                *i += 1;
            }
        }
        moved = true;
    }
    moved
}

/// What a copy holds of `frag`, copied from a document whose translations are hidden: no
/// hidden translation paragraphs (they weren't seen), and none at its end either (a selection
/// ending at the start of one), whose style a paste would otherwise bring along.
pub fn visible_fragment(frag: &mut wordcraft_doc::edit::Fragment) {
    if frag.blocks.len() < 2 {
        return;
    }
    let last = frag.blocks.len() - 1;
    let mut i = 0;
    frag.blocks.retain_mut(|b| {
        let keep = match b {
            Block::Para(p) if is_translation(p) && i == last => {
                p.props.style = None;
                true
            }
            Block::Para(p) if is_translation(p) && i > 0 => false,
            _ => true,
        };
        i += 1;
        keep
    });
}

/// A paste never makes translations: pasted paragraphs in the translation style lose it.
pub fn untranslated(frag: &mut wordcraft_doc::edit::Fragment) {
    for b in &mut frag.blocks {
        if let Block::Para(p) = b
            && is_translation(p)
        {
            p.props.style = None;
        }
    }
}

/// Where `pos` (in `before`) goes once its paragraph's text changed language (`after`): the same
/// share of the way through the paragraph, at a character boundary. Offsets don't carry over
/// between two languages' texts; a paragraph whose text didn't change keeps its offset.
pub fn carry_pos(before: &Document, after: &Document, pos: &Pos) -> Pos {
    let (Some(a), Some(b)) = (before.para(pos.story, &pos.path), after.para(pos.story, &pos.path)) else {
        return pos.clone();
    };
    if a.text == b.text {
        return pos.clone();
    }
    let chars_before = a.text.get(..pos.off.min(a.text.len())).map_or(0, |t| t.chars().count());
    let total = a.text.chars().count().max(1);
    let want = (chars_before as f64 / total as f64 * b.text.chars().count() as f64).round() as usize;
    let off = b.text.char_indices().nth(want).map_or(b.text.len(), |(i, _)| i);
    Pos { off, ..pos.clone() }
}

/// Revisions of mirror paragraphs, kept from one mirror to the next so unchanged pairs hit the
/// layout cache: (source rev, translation rev) → mirror paragraph rev.
#[derive(Default)]
pub struct MirrorRevs(HashMap<(u64, u64), u64>);

/// The mirror of `doc`: each paired source paragraph shows its translation instead. The block
/// structure is unchanged (the translations stay, hidden), so paths mean the same in both.
pub fn mirror(doc: &Document, revs: &mut MirrorRevs) -> Document {
    let mut m = doc.clone();
    let mut kept = HashMap::new();
    let tint = doc.styles.get(STYLE).and_then(|st| st.chr.color);
    for (s, t) in pairs(doc) {
        let (Some(src), Some(tr)) = (body_para(doc, s), body_para(doc, t)) else { continue };
        let key = (src.rev, tr.rev);
        let rev = revs.0.get(&key).copied().unwrap_or_else(next_rev);
        kept.insert(key, rev);
        if let Some(slot) = m.body.get_mut(s) {
            *slot = Arc::new(Block::Para(shown_translation(src, tr, rev, tint)));
        }
    }
    revs.0 = kept;
    m
}

/// The height of each top-level body paragraph's lines in `layout` (points; spacing left out),
/// summed over the pieces it was split into.
pub fn heights(layout: &DocLayout) -> HashMap<usize, f32> {
    let mut out = HashMap::new();
    for ((story, path), pieces) in &layout.index {
        if *story != StoryRef::Body || path.0.len() != 1 {
            continue;
        }
        let mut h = 0.0;
        for (page, item) in pieces {
            if let Some(Placed::Lines { para, l0, l1, .. }) = layout.pages.get(*page).and_then(|p| p.items.get(*item)) {
                h += para.lines.get(*l0..*l1).map(|ls| ls.iter().filter(|l| !l.beside).map(|l| l.height).sum::<f32>()).unwrap_or(0.0);
            }
        }
        out.insert(path.last(), h);
    }
    out
}

/// Padding that evens out each pair: (after the primary's source paragraphs, after the mirror's).
pub fn pads(pairs: &[(usize, usize)], primary: &DocLayout, mirror: &DocLayout) -> (HashMap<usize, f32>, HashMap<usize, f32>) {
    let (hp, hm) = (heights(primary), heights(mirror));
    let (mut pp, mut pm) = (HashMap::new(), HashMap::new());
    for (s, _) in pairs {
        let (a, b) = (hp.get(s).copied().unwrap_or(0.0), hm.get(s).copied().unwrap_or(0.0));
        if b > a + 0.01 {
            pp.insert(*s, b - a);
        } else if a > b + 0.01 {
            pm.insert(*s, a - b);
        }
    }
    (pp, pm)
}

/// The mirror document and its layout, padded to match the primary layout.
#[derive(Clone)]
pub struct MirrorView {
    pub doc: Arc<Document>,
    pub layout: Arc<DocLayout>,
}

/// A top-level body path, or `None` for paragraphs elsewhere (tables, headers, notes…).
pub fn top_block(story: StoryRef, path: &Path) -> Option<usize> {
    (story == StoryRef::Body && path.0.len() == 1).then(|| path.last())
}

/// The session's bilingual view state.
#[derive(Default)]
pub(crate) struct BiState {
    /// Whether the current layout was made for the bilingual view.
    pub on: bool,
    pub view: Option<MirrorView>,
    cache: LayoutCache,
    revs: MirrorRevs,
}

impl BiState {
    /// Lay out the mirror of `doc` and pad both it and `primary` (laid out with `opts`, unpadded)
    /// so they paginate alike; the padded primary layout is returned, the mirror kept in `view`.
    /// Without translations `primary` comes back as it is.
    pub fn lay_out(&mut self, doc: &Document, cache: &mut LayoutCache, opts: LayoutOptions, primary: Arc<DocLayout>) -> Arc<DocLayout> {
        let pairs = pairs(doc);
        if pairs.is_empty() {
            return primary;
        }
        let m = mirror(doc, &mut self.revs);
        let unpadded = wordcraft_layout::layout(&m, &mut self.cache, &opts);
        let (pp, pm) = pads(&pairs, &primary, &unpadded);
        let mirror_layout = if pm.is_empty() {
            unpadded
        } else {
            wordcraft_layout::layout(&m, &mut self.cache, &LayoutOptions { pad_after: Some(Arc::new(pm)), ..opts.clone() })
        };
        let primary = if pp.is_empty() {
            primary
        } else {
            Arc::new(wordcraft_layout::layout(doc, cache, &LayoutOptions { pad_after: Some(Arc::new(pp)), ..opts }))
        };
        self.view = Some(MirrorView { doc: Arc::new(m), layout: Arc::new(mirror_layout) });
        primary
    }
}
