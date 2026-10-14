<h1 align="center">WordDuo</h1>

<p align="center">
  <b>번역가와 통역사를 위한 한영 워드 프로세서</b><br>
  A Korean–English word processor for translators and interpreters
</p>

<p align="center">
  <a href="https://github.com/jinsu35/WordDuo/releases/latest"><b>⬇︎ macOS용 다운로드 (Apple 실리콘 · 인텔)</b></a>
</p>

WordDuo는 영어 문단과 그 한국어 번역을 **같은 자리에 겹쳐** 보여 줍니다. 원문과 번역을 번갈아
보거나, 반투명하게 섞어 보거나, 키 하나로 작업 언어를 바꿀 수 있습니다.

WordDuo는 [WordCraft](https://github.com/storytold/wordcraft)를 fork해서 만들었습니다.
WordCraft는 ArtCraft 팀이 Rust로 만든 오픈 소스 워드 프로세서입니다. WordDuo는 ArtCraft 팀의
공식 제품이 아니며, ArtCraft 팀이 만들거나 후원하는 프로젝트도 아닙니다.

## 사용법

| 동작 | 결과 |
|---|---|
| **⌥ Option 탭** | 작업 언어 전환 (영어 ↔ 한국어) |
| **⌥ Option 길게 누르기** | 누르고 있는 동안 다른 언어 미리 보기 |
| **⌥ 누른 채 클릭** | 그 언어로 전환하고, 클릭한 자리에 커서 |
| **트랙패드 두 손가락 돌리기** | 두 언어를 섞어 보기 (가운데를 넘기면 작업 언어도 바뀜) |
| 페이지 아래 슬라이더 | 마우스로 섞어 보기 |

- 문단마다 번역이 짝지어집니다. 번역은 같은 `.docx` 안에 숨긴 문단으로 저장되므로, Word에서도
  그대로 열립니다.
- 한쪽 언어로만 쓴 문단은 다른 언어 페이지에 회색으로 보입니다. 번역할 자리를 알려 주는 안내일
  뿐, 파일에는 저장되지 않습니다.
- 영어와 한국어를 번갈아 쓴 문서(EN, KO, EN, KO…)는 **보기 › 이중 언어 › 단락 짝짓기**로
  한 번에 짝지을 수 있습니다.

자세한 설명: [docs/bilingual.md](docs/bilingual.md)

## 설치 (macOS 11 이상)

1. [Releases](https://github.com/jinsu35/WordDuo/releases/latest)에서 `WordDuo-…-macos-universal.dmg`를 받습니다.
2. 열어서 WordDuo를 응용 프로그램 폴더로 끌어다 놓습니다.

Apple Developer ID로 서명하고 Apple 공증을 받았으므로 바로 실행됩니다.

## WordCraft에서 바뀐 것

- 이중 언어 보기: 겹쳐 보기, 섞어 보기, 작업 언어 전환, 미리 보기, 클릭 전환, 안내 글자
- 한글 입력 수정: 조합 중인 글자를 본문 안에 표시. 언어를 전환하거나 클릭할 때 조합 중인 글자가
  다른 자리에 다시 입력되지 않게 함
- 한국어 인터페이스와 macOS 한글 입력 수정은 WordCraft의 PR
  [#1127](https://github.com/storytold/wordcraft/pull/1127)과
  [#862](https://github.com/storytold/wordcraft/pull/862)(@ziozzang)에서 가져왔습니다.

## 소스에서 빌드

```sh
cargo run --release -p wordcraft -- 문서.docx
packaging/macos/package-wordduo.sh   # 서명·공증된 DMG
```

## 라이선스

WordCraft와 같은 MIT OR Apache-2.0입니다([LICENSE-MIT](LICENSE-MIT),
[LICENSE-APACHE](LICENSE-APACHE)). WordCraft의 저작권 고지와 서드파티 고지([NOTICE](NOTICE),
[ATTRIBUTION.md](ATTRIBUTION.md))를 그대로 유지합니다. ArtCraft의 이름과 로고는 ArtCraft 팀의
상표이며, 이 fork에서는 로고를 뺐습니다([docs/brand/LICENSE-brand.txt](docs/brand/LICENSE-brand.txt)).
WordCraft의 원래 README는 [README.WordCraft.md](README.WordCraft.md)에 있습니다.

---

**English.** WordDuo overlays each English paragraph and its Korean translation in the same place
on the page: tap ⌥ to switch the language you work in, hold ⌥ to peek at the other one, twist two
fingers on the trackpad to blend them. Translations are stored as hidden paragraphs in the same
`.docx`. WordDuo is a fork of [WordCraft](https://github.com/storytold/wordcraft) by the ArtCraft
team; it is not an ArtCraft product and is not endorsed by the ArtCraft team. MIT OR Apache-2.0.
