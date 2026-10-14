//! View › Bilingual: translations kept beside their paragraphs, and the view that shows either
//! language in the same place (see [`crate::bilingual`]).

use std::sync::Arc;

use serde_json::{Value, json};
use wordcraft_doc::para::Paragraph;
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Block, Pos, StoryRef};

use super::sel_result;
use crate::bilingual::{STYLE, ensure_style, is_korean, is_translation, pairs, source_of, top_block, translation_lang, translation_of};
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

fn toggle(s: &mut Session, v: &Value, f: fn(&mut crate::ViewState) -> &mut bool) -> CmdResult {
    let cur = *f(&mut s.view);
    *f(&mut s.view) = p::bool(v, "value").unwrap_or(!cur);
    Ok(json!({"value": *f(&mut s.view)}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        // Show each paragraph's translation in its place: flip to it (hold Option/Alt) or blend it in.
        CommandSpec::new("bilingual.view", "Bilingual View", "View › Bilingual", |s, v| toggle(s, v, |x| &mut x.bilingual))
            .params(r#"{"value"?: bool}"#)
            .pure(),
        CommandSpec::new("bilingual.mix", "Translation Blend", "View › Bilingual", mix)
            .params(r#"{"value": 0.0 (the text the document opened with) – 1.0 (its translation)}"#)
            .pure(),
        // The translations shown under their paragraphs, to write and edit them.
        CommandSpec::new("bilingual.edit", "Edit Translations", "View › Bilingual", |s, v| {
            let r = toggle(s, v, |x| &mut x.bilingual_edit);
            s.relayout();
            r
        })
        .params(r#"{"value"?: bool}"#)
        .pure(),
        CommandSpec::new("bilingual.translate", "Add Translation", "View › Bilingual", translate)
            .params(r#"{"text"?: string, "lang"?: BCP 47 tag (default: Korean for English text, English for Korean)}"#),
        CommandSpec::new("bilingual.pair", "Pair Paragraphs", "View › Bilingual", pair).params(r#"{"by"?: "language" (default) | "alternate"}"#),
        CommandSpec::new("bilingual.swap", "Swap Languages", "View › Bilingual", swap),
        // Tap Option (Alt) by itself, or turn the blend past the middle.
        CommandSpec::new("bilingual.switch", "Switch Language", "View › Bilingual", switch)
            .params(r#"{"value"?: bool (work in the translation), "keepBlend"?: bool}"#)
            .pure(),
        CommandSpec::new("bilingual.pairs", "List Translations", "View › Bilingual", list).pure(),
    ]
}

fn mix(s: &mut Session, v: &Value) -> CmdResult {
    let x = p::f32(v, "value").ok_or_else(|| CmdError::Params("`value` (number 0–1) is required".into()))?;
    s.view.bilingual_mix = x.clamp(0.0, 1.0);
    Ok(json!({"value": s.view.bilingual_mix}))
}

/// A language tag param: letters, digits and hyphens, at most 35 characters.
fn lang_param(v: &Value) -> Option<String> {
    p::str(v, "lang").filter(|t| !t.is_empty() && t.len() <= 35 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).map(str::to_string)
}

/// The translation paragraph for `text` in `lang`.
fn translation_para(text: &str, lang: &str) -> Paragraph {
    let clean: String = text.chars().map(|c| if matches!(c, '\n' | '\r' | '\u{0B}' | '\u{0C}') { ' ' } else { c }).collect();
    Paragraph::with_text(&clean, CharProps { lang: Some(lang.into()), ..Default::default() }).styled(STYLE)
}

/// Add (or, with `text`, set) the translation of the paragraph at the insertion point. Without
/// `text` the translations are shown for editing and the insertion point goes into this one.
fn translate(s: &mut Session, v: &Value) -> CmdResult {
    let focus = s.sel.focus.clone();
    let Some(block) = top_block(focus.story, &focus.path) else {
        return Err(CmdError::Failed("translations go with paragraphs of the body text (not tables, headers or notes)".into()));
    };
    let src = match source_of(&s.doc, block) {
        Some(src) => src,
        None if s.doc.body.get(block).and_then(|b| b.as_para()).is_some_and(is_translation) => {
            return Err(CmdError::Failed("this translation has no paragraph above it to translate".into()));
        }
        None => block,
    };
    let Some(source) = s.doc.body.get(src).and_then(|b| b.as_para()) else {
        return Err(CmdError::Failed("translations go with paragraphs, not tables".into()));
    };
    let lang = lang_param(v).unwrap_or_else(|| translation_lang(&source.plain_text()).to_string());
    let text = p::str(v, "text");
    ensure_style(&mut s.doc);
    let t = match translation_of(&s.doc, src) {
        Some(t) => {
            if let Some(text) = text
                && let Ok(tr) = s.doc.para_mut(StoryRef::Body, &wordcraft_doc::Path::top(t))
            {
                let fresh = translation_para(text, &lang);
                (tr.text, tr.runs, tr.objects) = (fresh.text, fresh.runs, fresh.objects);
                tr.touch();
            }
            t
        }
        None => {
            let mut tr = translation_para(text.unwrap_or_default(), &lang);
            // A section break stays at the end of the pair.
            if let Ok(sp) = s.doc.para_mut(StoryRef::Body, &wordcraft_doc::Path::top(src)) {
                tr.section = sp.section.take();
                sp.touch();
            }
            let at = src.saturating_add(1).min(s.doc.body.len());
            s.doc.body.insert(at, Arc::new(Block::Para(tr)));
            at
        }
    };
    if text.is_none() {
        if !s.view.bilingual_edit {
            s.view.bilingual_edit = true;
            s.relayout();
        }
        let off = s.doc.body.get(t).and_then(|b| b.as_para()).map(Paragraph::len).unwrap_or(0);
        s.sel = Selection::caret(Pos::body(t, off));
    }
    Ok(json!({"source": src, "translation": t, "lang": lang}))
}

fn para_of(b: &Block) -> Option<&Paragraph> {
    b.as_para()
}

fn blank(p: &Paragraph) -> bool {
    p.plain_text().trim().is_empty()
}

/// Make translations of paragraphs that follow the paragraph they translate: by `language`
/// (English then Korean, or Korean then English), or every `alternate` paragraph. Empty
/// paragraphs between the two are removed; paragraphs already paired are left as they are.
fn pair(s: &mut Session, v: &Value) -> CmdResult {
    let alternate = match p::str(v, "by") {
        None | Some("language") => false,
        Some("alternate") => true,
        Some(other) => return Err(CmdError::Params(format!("`by` must be \"language\" or \"alternate\", not {other:?}"))),
    };
    ensure_style(&mut s.doc);
    let body = std::mem::take(&mut s.doc.body);
    let mut out = Vec::with_capacity(body.len());
    let mut made = 0usize;
    let mut k = 0usize;
    while let Some(b) = body.get(k) {
        let Some(p) = para_of(b) else {
            out.push(b.clone());
            k += 1;
            continue;
        };
        // Already a pair (or a translation on its own): kept.
        if is_translation(p) || body.get(k + 1).and_then(|n| para_of(n)).is_some_and(is_translation) {
            out.push(b.clone());
            if !is_translation(p)
                && let Some(n) = body.get(k + 1)
            {
                out.push(n.clone());
                k += 1;
            }
            k += 1;
            continue;
        }
        if !blank(p) && p.section.is_none() {
            let mut j = k + 1;
            while body.get(j).and_then(|n| para_of(n)).is_some_and(|q| blank(q) && !is_translation(q) && q.section.is_none()) {
                j += 1;
            }
            let partner = body.get(j).and_then(|n| para_of(n)).filter(|q| {
                !blank(q)
                    && !is_translation(q)
                    && !body.get(j + 1).and_then(|n| para_of(n)).is_some_and(is_translation)
                    && (alternate || is_korean(&p.plain_text()) != is_korean(&q.plain_text()))
            });
            if let Some(q) = partner {
                let mut q = q.clone();
                q.props.style = Some(STYLE.into());
                q.touch();
                out.push(b.clone());
                out.push(Arc::new(Block::Para(q)));
                made += 1;
                k = j + 1;
                continue;
            }
        }
        out.push(b.clone());
        k += 1;
    }
    s.doc.body = out;
    s.sel = Selection::caret(s.doc.start_of(StoryRef::Body));
    Ok(json!({"paired": made, "pairs": pairs(&s.doc).len()}))
}

/// Swap every paragraph with its translation, as an undoable edit (see `bilingual.switch` for
/// changing the language being worked in).
fn swap(s: &mut Session, _: &Value) -> CmdResult {
    let n = crate::bilingual::swap_pairs(&mut s.doc);
    s.clamp_selection();
    sel_result(s).map(|mut r| {
        if let Some(m) = r.as_object_mut() {
            m.insert("swapped".into(), json!(n));
        }
        r
    })
}

/// Work in the other language: every paragraph and its translation change places, so the text
/// typed, printed and saved is the language shown. Not an undo step: the undo history is switched
/// too, so undoing takes back edits in either language. `value` is whether to work in the
/// translation (the language hidden when the document was opened); without it, the other one.
/// The blend goes all the way to the language worked in, unless `keepBlend` (a dial turned past
/// the middle).
fn switch(s: &mut Session, v: &Value) -> CmdResult {
    if pairs(&s.doc).is_empty() {
        return Err(CmdError::Failed("there are no translations to switch to".into()));
    }
    let to = p::bool(v, "value").unwrap_or(!s.view.bilingual_flipped);
    if to != s.view.bilingual_flipped {
        s.switch_bilingual();
    }
    if !p::bool(v, "keepBlend").unwrap_or(false) {
        s.view.bilingual_mix = if to { 1.0 } else { 0.0 };
    }
    Ok(json!({"translation": s.view.bilingual_flipped, "blend": s.view.bilingual_mix}))
}

fn list(s: &mut Session, _: &Value) -> CmdResult {
    let text = |b: usize| s.doc.body.get(b).and_then(|x| x.as_para()).map(Paragraph::plain_text).unwrap_or_default();
    let out: Vec<Value> =
        pairs(&s.doc).into_iter().map(|(a, b)| json!({"block": a, "text": text(a), "translationBlock": b, "translation": text(b)})).collect();
    Ok(json!({"pairs": out}))
}
