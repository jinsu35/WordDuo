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

#[test]
fn switching_language_edits_the_language_shown() {
    let mut s = Session::new(doc_of(&["Hello.", "Bye."]));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 0));
    run(&mut s, "bilingual.translate", json!({"text": "안녕."}));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(2, 0));
    run(&mut s, "text.insert", json!({"text": "Good "}));
    // Work in Korean: typed text goes into the Korean.
    let r = run(&mut s, "bilingual.switch", json!({}));
    assert_eq!((r["translation"].as_bool(), r["blend"].as_f64()), (Some(true), Some(1.0)));
    assert_eq!((body_text(&s, 0), body_text(&s, 1)), ("안녕.".into(), "Hello.".into()));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, "안녕".len()));
    run(&mut s, "text.insert", json!({"text": "하세요"}));
    assert_eq!(body_text(&s, 0), "안녕하세요.");
    // Back to English: the Korean edit stays with the Korean.
    run(&mut s, "bilingual.switch", json!({"value": false}));
    assert_eq!((body_text(&s, 0), body_text(&s, 1)), ("Hello.".into(), "안녕하세요.".into()));
    assert_eq!(s.view.bilingual_mix, 0.0);
    // Undo takes back the Korean typing, then the English, in the language shown now.
    run(&mut s, "edit.undo", json!({}));
    assert_eq!((body_text(&s, 0), body_text(&s, 1)), ("Hello.".into(), "안녕.".into()));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(body_text(&s, 2), "Bye.");
    assert_eq!(pairs(&s.doc), vec![(0, 1)]);
    // Turning the dial past the middle keeps the blend where it is.
    let r = run(&mut s, "bilingual.switch", json!({"value": true, "keepBlend": true}));
    assert_eq!(r["blend"].as_f64(), Some(0.0));
    // Without translations, a switch makes them; with nothing to translate, it fails.
    assert!(Session::new(doc_of(&["Only English."])).run("bilingual.switch", &json!({})).is_ok());
    let mut empty = Session::new(wordcraft_doc::Document::new());
    empty.doc.body.clear();
    assert!(empty.run("bilingual.switch", &json!({})).is_err());
}

/// An input method's composition is laid out with the text (so what follows makes room for it,
/// rather than being drawn over), without becoming part of the document.
#[test]
fn a_composition_makes_room_in_the_text_without_entering_the_document() {
    for bilingual in [false, true] {
        let mut s = Session::new(doc_of(&["Hello there friend."]));
        s.view.bilingual = bilingual;
        if bilingual {
            s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 0));
            run(&mut s, "bilingual.translate", json!({"text": "안 래 그래어조."}));
            run(&mut s, "bilingual.switch", json!({}));
        } else {
            run(&mut s, "select.all", json!({}));
            run(&mut s, "text.insert", json!({"text": "안 래 그래어조."}));
        }
        let at = "안 래 ".len();
        s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, at));
        let x_of = |l: &DocLayout, off: usize| l.caret_on(&wordcraft_doc::Pos::body(0, off), 0).map(|c| c.x).unwrap();
        let before = s.layout();
        let next = x_of(&before, at);
        s.set_preedit("녕");
        let shown = s.layout();
        let doc = s.display_doc().unwrap();
        assert_eq!(doc.body[0].as_para().unwrap().plain_text(), "안 래 녕그래어조.");
        assert_eq!(body_text(&s, 0), "안 래 그래어조.", "the document itself is unchanged");
        // "그" now starts after the composition.
        assert!(x_of(&shown, at + "녕".len()) > next + 5.0, "bilingual {bilingual}");
        // It goes when the composition ends (committed or cancelled).
        s.set_preedit("");
        assert!(s.display_doc().is_none());
        assert!((x_of(&s.layout(), at) - next).abs() < 0.01);
        // A selection shows none: typing replaces it.
        s.sel = crate::Selection { anchor: wordcraft_doc::Pos::body(0, 0), focus: wordcraft_doc::Pos::body(0, at) };
        s.set_preedit("녕");
        assert!(s.display_doc().is_none());
    }
}

/// Copy a sentence in one language, switch, paste: nothing in either language is lost. The
/// selection used to keep its offsets across the switch, selecting other text there unseen,
/// which the paste then replaced.
#[test]
fn switching_language_never_carries_a_selection_into_the_other_text() {
    let mut s = Session::new(doc_of(&["A good interpreter listens for intent.", "Second."]));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 0));
    run(&mut s, "bilingual.translate", json!({"text": "좋은 통역사는 의도를 듣는다."}));
    run(&mut s, "bilingual.switch", json!({}));
    run(&mut s, "select.text", json!({"text": "좋은 통역사는"}));
    run(&mut s, "edit.copy", json!({}));
    run(&mut s, "bilingual.switch", json!({}));
    assert!(s.sel.is_collapsed(), "a selection doesn't survive the switch");
    run(&mut s, "edit.paste", json!({}));
    let english = body_text(&s, 0);
    assert!(english.contains("좋은 통역사는") && english.replace("좋은 통역사는", "") == "A good interpreter listens for intent.", "{english}");
    run(&mut s, "bilingual.switch", json!({}));
    assert_eq!(body_text(&s, 0), "좋은 통역사는 의도를 듣는다.", "the Korean is whole");
    // The insertion point lands about as far through the other language's paragraph.
    let mut s = Session::new(doc_of(&["abcdefghij"]));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 0));
    run(&mut s, "bilingual.translate", json!({"text": "가나다라마바사아자차"}));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 5));
    run(&mut s, "bilingual.switch", json!({}));
    assert_eq!(s.sel.focus.off, "가나다라마".len());
    // An undo step's selection is carried too: undoing never selects the other language's text.
    run(&mut s, "text.insert", json!({"text": "!"}));
    run(&mut s, "bilingual.switch", json!({}));
    run(&mut s, "edit.undo", json!({}));
    assert!(s.sel.is_collapsed());
    assert!(s.doc.para_at(&s.sel.focus).is_some_and(|p| p.text.is_char_boundary(s.sel.focus.off)));
}

fn layout_of(s: &Session) -> Vec<(bool, String)> {
    s.doc.body.iter().map(|b| b.as_para().map(|p| (crate::bilingual::is_translation(p), p.plain_text())).unwrap_or_default()).collect()
}

/// Splitting a paragraph (Enter, a paste of paragraphs) keeps its translation with it; the new
/// paragraphs come after the translation rather than taking it over.
#[test]
fn a_split_paragraph_keeps_its_translation() {
    let pair = || {
        let mut s = Session::new(doc_of(&["abc def.", "Next."]));
        s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 0));
        run(&mut s, "bilingual.translate", json!({"text": "가나다 라마."}));
        s
    };
    let t = |b: bool, x: &str| (b, x.to_string());
    // Enter at the end, then a new sentence (which gets an empty translation of its own).
    let mut s = pair();
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, "abc def.".len()));
    run(&mut s, "text.newParagraph", json!({}));
    run(&mut s, "text.insert", json!({"text": "New."}));
    assert_eq!(layout_of(&s), vec![t(false, "abc def."), t(true, "가나다 라마."), t(false, "New."), t(true, ""), t(false, "Next.")]);
    assert_eq!(s.sel.focus.path.0, vec![2], "the insertion point is in the new paragraph");
    // Enter in the middle: the first half keeps it.
    let mut s = pair();
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 4));
    run(&mut s, "text.newParagraph", json!({}));
    assert_eq!(layout_of(&s), vec![t(false, "abc "), t(true, "가나다 라마."), t(false, "def."), t(true, ""), t(false, "Next.")]);
    // Enter at the start: the text moves down, and the translation with it.
    let mut s = pair();
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 0));
    run(&mut s, "text.newParagraph", json!({}));
    assert_eq!(layout_of(&s), vec![t(false, ""), t(true, ""), t(false, "abc def."), t(true, "가나다 라마."), t(false, "Next.")]);
    // Undo puts it back as it was.
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(layout_of(&s), vec![t(false, "abc def."), t(true, "가나다 라마."), t(false, "Next.")]);
}

/// Copying a whole paragraph (its selection runs to the start of the hidden translation after
/// it) and pasting it at the end of another: no empty translation appears, and every
/// translation stays with its paragraph.
#[test]
fn copying_paragraphs_never_moves_or_makes_translations() {
    let mut s = Session::new(doc_of(&["One.", "Two."]));
    for (b, ko) in [(0, "하나."), (2, "둘.")] {
        s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(b, 0));
        run(&mut s, "bilingual.translate", json!({"text": ko}));
    }
    run(&mut s, "bilingual.switch", json!({}));
    run(&mut s, "select.text", json!({"text": "둘."}));
    run(&mut s, "select.paragraph", json!({}));
    run(&mut s, "edit.copy", json!({}));
    assert!(!s.clipboard_text.contains("Two"), "the hidden English isn't copied: {:?}", s.clipboard_text);
    run(&mut s, "bilingual.switch", json!({}));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, "One.".len()));
    run(&mut s, "edit.paste", json!({}));
    let l = layout_of(&s);
    assert_eq!(l.first(), Some(&(false, "One.둘.".to_string())), "{l:?}");
    assert_eq!(l.get(1), Some(&(true, "하나.".to_string())), "{l:?}");
    assert!(
        l.iter().all(|(tr, x)| !tr || x.is_empty() || x == "하나." || x == "둘."),
        "only the pasted paragraph's empty translations are new: {l:?}"
    );
    assert!(pairs(&s.doc).iter().any(|(_, t)| body_text(&s, *t) == "둘."), "{l:?}");
}

/// Paragraphs written in one language are in that language only: the user's report, Korean
/// typed on new lines at the end showed on the English page too.
#[test]
fn new_paragraphs_are_in_the_language_written_only() {
    let mut s = Session::new(doc_of(&["Hello."]));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 0));
    run(&mut s, "bilingual.translate", json!({"text": "안녕."}));
    run(&mut s, "bilingual.switch", json!({}));
    s.sel = crate::Selection::caret(s.doc.end_of(StoryRef::Body));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, "안녕.".len()));
    for _ in 0..3 {
        run(&mut s, "text.newParagraph", json!({}));
    }
    run(&mut s, "text.insert", json!({"text": "좋다"}));
    let korean: Vec<String> =
        s.doc.body.iter().filter_map(|b| b.as_para()).filter(|p| !crate::bilingual::is_translation(p)).map(|p| p.plain_text()).collect();
    assert!(korean.contains(&"좋다".to_string()));
    run(&mut s, "bilingual.switch", json!({}));
    let english: Vec<String> =
        s.doc.body.iter().filter_map(|b| b.as_para()).filter(|p| !crate::bilingual::is_translation(p)).map(|p| p.plain_text()).collect();
    assert!(!english.iter().any(|t| t.contains("좋다")), "the English page doesn't show it: {english:?}");
    assert_eq!(english.first().map(String::as_str), Some("Hello."));
    // Both pages lay out alike: the new lines are empty lines on the English page.
    let primary = s.layout();
    let m = s.bilingual_mirror().unwrap();
    assert_eq!(primary.pages.len(), m.layout.pages.len());
}

/// The same with translations paired from plain text, which carry no language tag (the demo
/// document): a new paragraph got no translation, so it showed, and was edited, on both pages.
#[test]
fn new_paragraphs_get_a_translation_without_a_language_tag_too() {
    let mut s = Session::new(doc_of(&["Hello there.", "안녕하세요 여러분."]));
    run(&mut s, "bilingual.pair", json!({}));
    assert_eq!(pairs(&s.doc), vec![(0, 1)]);
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, "Hello there.".len()));
    run(&mut s, "text.newParagraph", json!({}));
    run(&mut s, "text.insert", json!({"text": "Hello again."}));
    assert_eq!(layout_of(&s).get(2), Some(&(false, "Hello again.".to_string())));
    assert_eq!(layout_of(&s).get(3), Some(&(true, String::new())), "it has a translation of its own");
    // Written over on the Korean page, the English stays.
    run(&mut s, "bilingual.switch", json!({}));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(2, 0));
    run(&mut s, "text.insert", json!({"text": "다시 안녕."}));
    run(&mut s, "bilingual.switch", json!({}));
    assert_eq!(layout_of(&s).get(2), Some(&(false, "Hello again.".to_string())));
    assert_eq!(layout_of(&s).get(3), Some(&(true, "다시 안녕.".to_string())));
}

/// An empty paragraph whose other language has text shows that text greyed as a hint, on either
/// page; typing replaces it, and it is never part of the document.
#[test]
fn an_untranslated_paragraph_shows_the_other_language_as_a_hint() {
    let mut s = Session::new(doc_of(&["Hello there.", "Bye."]));
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::body(0, 0));
    run(&mut s, "bilingual.translate", json!({"text": ""}));
    // English page: the translation is empty, so peeking shows the English greyed.
    let m = s.bilingual_mirror().unwrap();
    let shown = m.doc.body[0].as_para().unwrap();
    assert_eq!(shown.plain_text(), "Hello there.");
    assert_eq!(shown.runs[0].props.color, Some(wordcraft_doc::props::TextColor::Rgb(wordcraft_doc::props::Rgb(0x9A, 0x9A, 0x9A))));
    // Korean page: the paragraph is empty, and shows the English greyed in its place.
    run(&mut s, "bilingual.switch", json!({}));
    assert_eq!(body_text(&s, 0), "", "the document holds no hint");
    let doc = s.display_doc().unwrap();
    assert_eq!(doc.body[0].as_para().unwrap().plain_text(), "Hello there.");
    // Both pages still lay out alike.
    let (primary, m) = (s.layout(), s.bilingual_mirror().unwrap());
    assert_eq!(primary.pages.len(), m.layout.pages.len());
    // A click on the hint puts the insertion point in the empty paragraph.
    s.run("caret.set", &json!({"pos": {"story": "body", "path": [0], "off": 5}})).ok();
    assert_eq!((s.sel.focus.path.0.clone(), s.sel.focus.off), (vec![0], 0));
    // Composing there shows the composition, not the hint.
    s.set_preedit("안");
    assert_eq!(s.display_doc().unwrap().body[0].as_para().unwrap().plain_text(), "안");
    s.set_preedit("");
    // Typing replaces it.
    run(&mut s, "text.insert", json!({"text": "안녕하세요."}));
    assert!(s.display_doc().is_none_or(|d| d.body[0].as_para().unwrap().plain_text() == "안녕하세요."), "no hint once written");
    assert_eq!(body_text(&s, 0), "안녕하세요.");
    // Saved, the file holds no hint either.
    let bytes = crate::io::save_bytes("x.docx", &s.doc).unwrap();
    let back = crate::io::open_bytes("x.docx", &bytes).unwrap();
    assert!(!back.plain_text(StoryRef::Body).contains("9A9A9A"));
    assert_eq!(back.body[0].as_para().unwrap().plain_text(), "안녕하세요.");
}

/// The user's report: a new document, ⌥ did nothing. A document without translations switches
/// too: every paragraph gets an empty translation to write the other language in, and the
/// undo history keeps working in either language.
#[test]
fn a_new_document_switches_to_write_the_other_language() {
    for blank in [true, false] {
        let mut s = Session::new(wordcraft_doc::Document::new());
        if blank {
            run(&mut s, "file.new", json!({}));
        }
        run(&mut s, "text.insert", json!({"text": "Hello."}));
        run(&mut s, "text.newParagraph", json!({}));
        run(&mut s, "text.insert", json!({"text": "Bye."}));
        run(&mut s, "bilingual.switch", json!({}));
        assert!(s.view.bilingual_flipped, "working in the other language");
        assert_eq!(pairs(&s.doc).len(), 2, "every paragraph has a translation");
        let shown: Vec<String> =
            s.doc.body.iter().filter_map(|b| b.as_para()).filter(|p| !crate::bilingual::is_translation(p)).map(|p| p.plain_text()).collect();
        assert_eq!(shown, vec!["", ""], "the other language is empty, to write");
        assert_eq!(s.sel.focus.path.0, vec![2], "still in the second paragraph");
        run(&mut s, "text.insert", json!({"text": "잘 가."}));
        // Undo in either language, then back.
        run(&mut s, "edit.undo", json!({}));
        run(&mut s, "edit.undo", json!({}));
        run(&mut s, "bilingual.switch", json!({}));
        assert!(!s.view.bilingual_flipped);
        let english: Vec<String> =
            s.doc.body.iter().filter_map(|b| b.as_para()).filter(|p| !crate::bilingual::is_translation(p)).map(|p| p.plain_text()).collect();
        assert_eq!(english.first().map(String::as_str), Some("Hello."), "{english:?}");
        assert!(!pairs(&s.doc).is_empty());
        run(&mut s, "edit.redo", json!({}));
        run(&mut s, "edit.redo", json!({}));
        run(&mut s, "bilingual.switch", json!({}));
        assert!(body_texts(&s).contains(&"잘 가.".to_string()), "{:?}", body_texts(&s));
    }
}

fn body_texts(s: &Session) -> Vec<String> {
    s.doc.body.iter().filter_map(|b| b.as_para()).filter(|p| !crate::bilingual::is_translation(p)).map(|p| p.plain_text()).collect()
}
