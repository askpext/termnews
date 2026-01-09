# TermNews

![Crates.io](https://img.shields.io/crates/v/termnews?style=flat-square&color=orange) ![License](https://img.shields.io/crates/l/termnews?style=flat-square)


**Stop doomscrolling. Start reading.**

TermNews is a high-performance terminal news reader built for nerds who want to stay informed without leaving the command line. It aggregates your favorite RSS feeds, strips away the web's clutter (ads, popups, paywalls), and renders clean, readable text instantly.

<p align="center">
  <img width="360" alt="0109(4)" src="https://github.com/user-attachments/assets/5a02f11a-1445-4cab-b8e1-6383acd5a88e" />
  <img width="360" alt="0109(3)" src="https://github.com/user-attachments/assets/062b8543-bd98-4349-8335-754508f07c0b" />
</p>
<p align="center">
  <img width="360" alt="0109(2)" src="https://github.com/user-attachments/assets/9a44ec9e-e29d-4f4a-9dcf-bfb9a0274626" />
  <img width="360" alt="0109(1)" src="https://github.com/user-attachments/assets/aa72ff5a-8278-4311-ac49-344fa7741ae4" />
</p>

---

## Why TermNews?

* **Zero Latency:** Written in Rust for instant startup and navigation.
* **Reader Mode:** Automatically extracts article content and removes ads/bloat.
* **Privacy First:** No tracking pixels, no cookies, just HTTP requests.
* **Vim-Native:** Navigate with `j/k`, switch tabs with `1-9`.
* **Hackable:** Simple TOML configuration. Combine multiple feeds into a single stream.

---

## Installation

### From Crates.io (Recommended)

```bash
cargo install termnews
```

### From Source

```bash
git clone https://github.com/askpext/termnews.git
cd termnews
cargo install --path .
```

---

## Controls

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate Up / Down |
| `Enter` | Read Article (Reader Mode) |
| `1` - `9` | Switch Tabs |
| `r` | Refresh Feeds |
| `s` | Save Article to `saved_news.md` |
| `c` | Edit Config (Opens in default editor) |
| `o` | Open in Browser |
| `q` | Quit / Back |

---

## Configuration

TermNews is opinionated out of the box, but fully customizable. Press `c` inside the app to edit your `config.toml`.

**Power User Tip:** You can aggregate multiple sources into a single tab (e.g., a "Tech Firehose").

```toml
# ~/.config/termnews/config.toml

[[feeds]]
name = "⚡ Fast Tech"
urls = [
    "https://www.techmeme.com/feed.xml",
    "https://news.google.com/rss/search?q=technology"
]

[[feeds]]
name = "🦀 Rust"
urls = [
    "https://blog.rust-lang.org/feed.xml",
    "https://this-week-in-rust.org/rss.xml"
]

[[feeds]]
name = "🌍 World"
urls = [
    "https://www.aljazeera.com/xml/rss/all.xml"
]
```
## Legal Disclaimer

**TermNews is a content aggregator and terminal-based viewer.**
All articles, headlines, and content displayed by this tool remain the property of their respective owners. This tool fetches publicly available RSS feeds and formats them for personal reading, acting as a user-agent (browser). Users are responsible for adhering to the Terms of Service of the sources they access.

**No Warranty:**
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY.