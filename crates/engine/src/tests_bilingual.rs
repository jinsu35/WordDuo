//! Bilingual documents: translations beside their paragraphs and the view that lays both
//! languages out in the same place.

use std::sync::Arc;

use serde_json::json;
use wordcraft_doc::para::Paragraph;
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Block, Document, StoryRef};
use wordcraft_layout::{DocLayout, Placed};

use crate::bilingual::{STYLE, pairs};
use crate::{Session, cmd};

fn doc_of(paras: &[&str]) -> Document {
    let mut d = Document::new();
    d.body = paras.iter().map(|t| Arc::new(Block::Para(Paragraph::with_text(t, CharProps::default())))).collect();
    d
}

fn run(s: &mut Session, id: &str, v: serde_json::Value) -> serde_json::Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// (page, top) of the first line of top-level body block `b`.
fn top_of(l: &DocLayout, b: usize) -> Option<(usize, f32)> {
    for (pi, page) in l.pages.iter().enumerate() {
        for it in &page.items {
            if let Placed::Lines { story: StoryRef::Body, path, y, .. } = it
                && path.0 == [b as u32]
            {
                return Some((pi, *y));
            }
        }
    }
    None
}

fn body_text(s: &Session, b: usize) -> String {
    s.doc.body.get(b).and_then(|x| x.as_para()).map(Paragraph::plain_text).unwrap_or_default()
}

#[test]
fn a_translation_is_hidden_and_takes_no_room() {
    let mut plain = Session::new(doc_of(&["The quick brown fox.", "Jumps over the lazy dog."]));
    let mut s = Session::new(doc_of(&["The quick brown fox.", "Jumps over the lazy dog."]));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 0));
    let r = run(&mut s, "bilingual.translate", json!({"text": "빠른 갈색 여우."}));
    assert_eq!(r["translation"], 1);
    assert_eq!(r["lang"], "ko-KR");
    assert_eq!(pairs(&s.doc), vec![(0, 1)]);
    assert_eq!(body_text(&s, 1), "빠른 갈색 여우.");
    assert_eq!(s.doc.body[1].as_para().and_then(|p| p.props.style.as_deref()), Some(STYLE));
    // The paragraph after it is where it was without the translation (the bilingual view off:
    // it would pad the paragraph to the translation's height).
    s.view.bilingual = false;
    let (a, b) = (plain.layout(), s.layout());
    assert_eq!(top_of(&a, 1), top_of(&b, 2));
    // Shown for editing, it takes its line.
    run(&mut s, "bilingual.edit", json!({"value": true}));
    let shown = s.layout();
    assert!(top_of(&shown, 2).unwrap().1 > top_of(&b, 2).unwrap().1);
}

#[test]
fn translate_without_text_opens_the_translation_for_editing() {
    let mut s = Session::new(doc_of(&["안녕하세요.", "Next."]));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 3));
    let r = run(&mut s, "bilingual.translate", json!({}));
    assert_eq!(r["lang"], "en-US", "Korean text is translated into English");
    assert!(s.view.bilingual_edit);
    assert_eq!(s.sel.focus.path.0, vec![1]);
    run(&mut s, "text.insert", json!({"text": "Hello."}));
    assert_eq!(body_text(&s, 1), "Hello.");
    assert_eq!(pairs(&s.doc), vec![(0, 1)]);
    // From inside the translation, the command finds the same pair.
    let r = run(&mut s, "bilingual.translate", json!({"text": "Hi."}));
    assert_eq!((r["source"].as_u64(), r["translation"].as_u64()), (Some(0), Some(1)));
    assert_eq!(body_text(&s, 1), "Hi.");
    assert_eq!(s.doc.body.len(), 3);
}

#[test]
fn both_languages_sit_in_the_same_place() {
    let long_ko = "이것은 아주 긴 번역 문장입니다. ".repeat(20);
    let mut s = Session::new(doc_of(&["Short.", "After the first pair.", "Also short.", "The end."]));
    for (b, t) in [(0, long_ko.as_str()), (3, "짧음.")] {
        // (block 3 after the first translation is inserted: "Also short.")
        s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(b, 0));
        run(&mut s, "bilingual.translate", json!({"text": t}));
    }
    assert_eq!(pairs(&s.doc), vec![(0, 1), (3, 4)]);
    assert!(s.view.bilingual, "on by default");
    assert!(Session::new(doc_of(&["No translations."])).bilingual_mirror().is_none(), "nothing to mirror");
    let primary = s.layout();
    let m = s.bilingual_mirror().unwrap();
    assert_eq!(m.doc.body.len(), s.doc.body.len(), "same block structure");
    assert_eq!(m.doc.body[0].as_para().unwrap().plain_text(), long_ko);
    assert_eq!(m.doc.body[3].as_para().unwrap().plain_text(), "짧음.");
    assert_eq!(primary.pages.len(), m.layout.pages.len());
    for b in [0, 2, 3, 5] {
        let (p, q) = (top_of(&primary, b).unwrap(), top_of(&m.layout, b).unwrap());
        assert_eq!(p.0, q.0, "block {b} on the same page");
        assert!((p.1 - q.1).abs() < 0.01, "block {b} at the same height: {p:?} vs {q:?}");
    }
    // The long translation pushed the text after it down in the primary view too.
    let mut off = Session::new(s.doc.clone());
    off.view.bilingual = false;
    assert!(top_of(&primary, 2).unwrap().1 > top_of(&off.layout(), 2).unwrap().1 + 20.0);
    // Editing the text keeps the two in step.
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(3, 0));
    run(&mut s, "text.insert", json!({"text": &"More words here. ".repeat(10)}));
    let (primary, m) = (s.layout(), s.bilingual_mirror().unwrap());
    let (p, q) = (top_of(&primary, 5).unwrap(), top_of(&m.layout, 5).unwrap());
    assert!((p.1 - q.1).abs() < 0.01 && p.0 == q.0, "{p:?} vs {q:?}");
}

#[test]
fn pairing_by_language_and_alternately() {
    let mut s = Session::new(doc_of(&["Hello.", "", "안녕하세요.", "World.", "세계.", "Alone.", "Also English."]));
    let r = run(&mut s, "bilingual.pair", json!({}));
    assert_eq!(r["paired"], 2);
    assert_eq!(pairs(&s.doc), vec![(0, 1), (2, 3)]);
    assert_eq!(body_text(&s, 1), "안녕하세요.", "the empty paragraph between went");
    assert_eq!(s.doc.body.len(), 6);
    // Running it again changes nothing.
    assert_eq!(run(&mut s, "bilingual.pair", json!({}))["paired"], 0);
    let r = run(&mut s, "bilingual.pair", json!({"by": "alternate"}));
    assert_eq!(r["paired"], 1);
    assert_eq!(pairs(&s.doc), vec![(0, 1), (2, 3), (4, 5)]);
    assert!(s.run("bilingual.pair", &json!({"by": "nonsense"})).is_err());
}

#[test]
fn swap_languages_and_undo() {
    let mut s = Session::new(doc_of(&["One.", "Two."]));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 0));
    run(&mut s, "bilingual.translate", json!({"text": "하나."}));
    run(&mut s, "bilingual.swap", json!({}));
    assert_eq!((body_text(&s, 0), body_text(&s, 1)), ("하나.".into(), "One.".into()));
    assert_eq!(pairs(&s.doc), vec![(0, 1)]);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!((body_text(&s, 0), body_text(&s, 1)), ("One.".into(), "하나.".into()));
}

#[test]
fn translations_survive_a_docx_round_trip() {
    let mut s = Session::new(doc_of(&["Hello.", "Bye."]));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 0));
    run(&mut s, "bilingual.translate", json!({"text": "안녕하세요."}));
    let bytes = crate::io::save_bytes("x.docx", &s.doc).unwrap();
    let back = crate::io::open_bytes("x.docx", &bytes).unwrap();
    assert_eq!(pairs(&back), vec![(0, 1)]);
    assert_eq!(back.body[1].as_para().unwrap().plain_text(), "안녕하세요.");
    assert_eq!(back.styles.get(STYLE).and_then(|st| st.chr.hidden), Some(true));
    // On the page it takes no room: the next paragraph starts where the first one's space ends.
    let (mut t, mut u) = (Session::new(back), Session::new(doc_of(&["Hello.", "Bye."])));
    t.view.bilingual = false;
    assert_eq!(top_of(&t.layout(), 2), top_of(&u.layout(), 1));
}

#[test]
fn bilingual_commands_never_panic_on_odd_params() {
    for id in cmd::bilingual::specs().iter().map(|c| c.id) {
        for v in [json!(null), json!({"value": "x", "text": 5, "lang": "x".repeat(99), "by": 1}), json!({"value": f64::MAX, "text": "a\nb"})] {
            let mut s = Session::new(doc_of(&["A", "B"]));
            let _ = s.run(id, &v);
            let _ = s.layout();
        }
    }
    // In a table cell, inside a header, at the end of the document: an error or a pair, never a panic.
    let mut s = Session::new(doc_of(&["A"]));
    run(&mut s, "insert.table", json!({"rows": 1, "cols": 1}));
    let _ = s.run("bilingual.translate", &json!({}));
    s.sel = crate::Selection::caret(s.doc.end_of(StoryRef::Body));
    let _ = s.run("bilingual.translate", &json!({"text": "끝"}));
}
