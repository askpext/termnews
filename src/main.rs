use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, widgets::*, widgets::BorderType};
use std::{io::{self, Write}, sync::Arc, fs};
use tokio::sync::Mutex;
use rss::Channel;
use serde::Deserialize;
use directories::ProjectDirs;

// --- 1. CONFIGURATION ---

#[derive(Debug, Deserialize, Clone)]
struct FeedConfig {
    name: String,
    urls: Vec<String>, // Support multiple URLs per tab
}

#[derive(Debug, Deserialize, Clone)]
struct Config {
    feeds: Vec<FeedConfig>,
}

impl Config {
    fn load() -> Config {
        // 1. Try local "config.toml"
        if let Ok(contents) = fs::read_to_string("config.toml") {
            if let Ok(config) = toml::from_str(&contents) {
                return config;
            }
        }

        // 2. Try "~/.config/termnews/config.toml"
        if let Some(proj_dirs) = ProjectDirs::from("", "", "termnews") {
            let path = proj_dirs.config_dir().join("config.toml");
            if let Ok(contents) = fs::read_to_string(path) {
                if let Ok(config) = toml::from_str(&contents) {
                    return config;
                }
            }
        }

        // 3. Fallback Defaults (OPINIONATED STARTING PACK)
        Config {
            feeds: vec![
                FeedConfig { 
                    name: "Tech Hub".to_string(), 
                    urls: vec![
                        "https://techcrunch.com/feed/".to_string(),
                        "https://www.theverge.com/rss/index.xml".to_string(),
                        "https://wired.com/feed/rss".to_string(),
                    ] 
                },
                FeedConfig { 
                    name: "World".to_string(), 
                    urls: vec![
                        "http://feeds.bbci.co.uk/news/world/rss.xml".to_string(),
                        "https://www.aljazeera.com/xml/rss/all.xml".to_string(),
                    ] 
                },
                FeedConfig { 
                    name: "Sports".to_string(), 
                    urls: vec![
                        "https://www.espn.com/espn/rss/news".to_string(),
                    ] 
                },
                FeedConfig { 
                    name: "Rust".to_string(), 
                    urls: vec!["https://blog.rust-lang.org/feed.xml".to_string()] 
                },
            ]
        }
    }
}

// --- 2. DATA STRUCTURES ---

#[derive(Clone, Debug)]
struct NewsItem {
    title: String,
    source: String,
    score: Option<String>,
    url: Option<String>,
}

#[derive(PartialEq)]
enum ViewMode {
    List,
    Reading,
}

struct App {
    pub items: Vec<NewsItem>,
    pub state: ListState,
    pub is_loading: bool,
    pub current_tab: usize,
    pub config: Config, 
    
    // Reader Mode
    pub view_mode: ViewMode,
    pub article_text: String, 
    pub scroll: u16,          
    pub status_message: Option<String>,
}

impl App {
    fn new(config: Config) -> App {
        App {
            items: vec![],
            state: ListState::default(),
            is_loading: true,
            current_tab: 0,
            config,
            view_mode: ViewMode::List,
            article_text: String::new(),
            scroll: 0,
            status_message: None,
        }
    }

    fn next(&mut self) {
        if self.items.is_empty() { return; }
        let i = match self.state.selected() {
            Some(i) => if i >= self.items.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.state.select(Some(i));
        self.status_message = None; 
    }

    fn previous(&mut self) {
        if self.items.is_empty() { return; }
        let i = match self.state.selected() {
            Some(i) => if i == 0 { self.items.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.state.select(Some(i));
        self.status_message = None; 
    }
}

// --- 3. LOGIC HELPERS ---

fn save_article(item: &NewsItem) -> String {
    if let Some(url) = &item.url {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("saved_news.md")
            .unwrap_or_else(|_| std::fs::File::create("saved_news.md").unwrap());

        if let Err(_) = writeln!(file, "- [{}]({})", item.title, url) {
            return "Failed to save file.".to_string();
        }
        return format!("Saved: \"{}\"", item.title);
    }
    "No URL to save.".to_string()
}

// Helper to Open Config in Editor
fn open_config_in_editor() -> String {
    let path = if let Some(proj_dirs) = ProjectDirs::from("", "", "termnews") {
        proj_dirs.config_dir().join("config.toml")
    } else {
        std::path::PathBuf::from("config.toml")
    };

    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        // Template for new users
        let template = r#"
[[feeds]]
name = "Tech Hub"
urls = [
    "https://techcrunch.com/feed/",
    "https://www.theverge.com/rss/index.xml"
]

[[feeds]]
name = "Crypto"
urls = [
    "https://cointelegraph.com/rss"
]
"#;
        if let Err(_) = fs::write(&path, template) {
            return "Failed to create config file.".to_string();
        }
    }

    match open::that(&path) {
        Ok(_) => "Opened config file.".to_string(),
        Err(_) => "Could not open config file.".to_string(),
    }
}

// --- 4. NETWORK ENGINES ---

async fn fetch_rss(url: &str) -> Result<Vec<NewsItem>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(5)).build()?;
    let content = client.get(url).send().await?.bytes().await?;
    let channel = Channel::read_from(&content[..]).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    
    let items = channel.items().iter().take(15).map(|item| {
        NewsItem {
            title: item.title().unwrap_or("No Title").to_string(),
            source: channel.title().to_string(),
            score: Some("RSS".to_string()),
            url: item.link().map(|s| s.to_string()),
        }
    }).collect();

    Ok(items)
}

// Multi-Source Fetcher (Parallel Aggregation)
async fn fetch_data_for_tab(config: Config, tab_index: usize) -> Result<Vec<NewsItem>, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(feed_group) = config.feeds.get(tab_index) {
        // Spawn a thread for EVERY URL in this tab
        let fetch_tasks: Vec<_> = feed_group.urls.iter().map(|url| {
            let url = url.clone();
            tokio::spawn(async move {
                fetch_rss(&url).await.ok() // Ignore failures, keep working feeds
            })
        }).collect();

        let mut all_items = Vec::new();
        for task in futures::future::join_all(fetch_tasks).await {
            if let Ok(Some(mut items)) = task {
                all_items.append(&mut items);
            }
        }
        return Ok(all_items);
    }
    Ok(vec![])
}

async fn fetch_and_clean_article(url: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let resp = client.get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .send()
        .await?;

    if let Some(content_type) = resp.headers().get("content-type") {
        let ct = content_type.to_str().unwrap_or("");
        if ct.contains("application/pdf") || ct.contains("image/") {
            return Ok("⚠️  Media File. Press 'o' to open in browser.".to_string());
        }
    }

    let final_url = resp.url().clone();
    let html = resp.text().await?;
    let mut cursor = io::Cursor::new(html.clone());
    
    match readability::extractor::extract(&mut cursor, &final_url) {
        Ok(extractor) => {
            let title = format!("# {}\n\n", extractor.title);
            let text = html2text::from_read(extractor.content.as_bytes(), 100);
            if text.trim().len() < 50 {
                 return Ok(format!("{}\n\n⚠️  Empty Content. Press 'o' to open in browser.", title));
            }
            Ok(format!("{}{}", title, text))
        },
        Err(_) => {
            let text = html2text::from_read(html.as_bytes(), 100);
            Ok(format!("⚠️  Extraction Failed. Raw text:\n\n{}", text))
        }
    }
}

// --- 5. MAIN LOOP ---

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load();
    let app = Arc::new(Mutex::new(App::new(config.clone())));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    trigger_refresh(app.clone(), config.clone(), 0);

    loop {
        {
            let mut locked_app = app.lock().await;
            terminal.draw(|f| ui(f, &mut locked_app))?;
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let mut locked_app = app.lock().await;
                    let current_config = locked_app.config.clone();

                    // Global Quit
                    if key.code == KeyCode::Char('q') {
                        if locked_app.view_mode == ViewMode::Reading {
                            locked_app.view_mode = ViewMode::List; 
                        } else {
                            break; 
                        }
                    }

                    match locked_app.view_mode {
                        ViewMode::List => {
                            match key.code {
                                KeyCode::Char('j') | KeyCode::Down => locked_app.next(),
                                KeyCode::Char('k') | KeyCode::Up => locked_app.previous(),
                                
                                // Dynamic Tab Switching (1-9)
                                KeyCode::Char(c) if c.is_digit(10) && c != '0' => {
                                    let idx = (c.to_digit(10).unwrap() - 1) as usize;
                                    if idx < current_config.feeds.len() {
                                        switch_tab(&mut locked_app, app.clone(), current_config, idx);
                                    }
                                }
                                
                                KeyCode::Char('r') => {
                                    let tab = locked_app.current_tab;
                                    locked_app.status_message = Some("Refreshing...".to_string());
                                    drop(locked_app); 
                                    trigger_refresh(app.clone(), current_config, tab);
                                }
                                KeyCode::Char('s') => {
                                    if let Some(i) = locked_app.state.selected() {
                                        if let Some(item) = locked_app.items.get(i).cloned() {
                                            let msg = save_article(&item);
                                            locked_app.status_message = Some(msg);
                                        }
                                    }
                                }
                                // NEW: Config Editor
                                KeyCode::Char('c') => {
                                    let msg = open_config_in_editor();
                                    locked_app.status_message = Some(msg);
                                }
                                KeyCode::Enter => {
                                    if let Some(i) = locked_app.state.selected() {
                                        if let Some(item) = locked_app.items.get(i).cloned() {
                                            if let Some(url) = item.url {
                                                locked_app.is_loading = true;
                                                locked_app.view_mode = ViewMode::Reading;
                                                locked_app.article_text = "Loading article...".to_string();
                                                locked_app.scroll = 0;
                                                locked_app.status_message = None;
                                                drop(locked_app); 
                                                
                                                let app_bg = app.clone();
                                                tokio::spawn(async move {
                                                    let text = match fetch_and_clean_article(url).await {
                                                        Ok(t) => t,
                                                        Err(_) => "Failed to load article.".to_string(),
                                                    };
                                                    let mut locked = app_bg.lock().await;
                                                    locked.article_text = text;
                                                    locked.is_loading = false;
                                                });
                                            }
                                        }
                                    }
                                }
                                KeyCode::Char('o') => {
                                    if let Some(i) = locked_app.state.selected() {
                                        if let Some(item) = locked_app.items.get(i) {
                                            if let Some(url) = &item.url {
                                                let _ = open::that(url);
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        },
                        ViewMode::Reading => {
                            match key.code {
                                KeyCode::Esc | KeyCode::Backspace => locked_app.view_mode = ViewMode::List,
                                KeyCode::Char('j') | KeyCode::Down => locked_app.scroll = locked_app.scroll.saturating_add(1),
                                KeyCode::Char('k') | KeyCode::Up => locked_app.scroll = locked_app.scroll.saturating_sub(1),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn switch_tab(app_state: &mut App, app_arc: Arc<Mutex<App>>, config: Config, tab_index: usize) {
    if app_state.current_tab != tab_index {
        app_state.current_tab = tab_index;
        app_state.items.clear();
        app_state.is_loading = true;
        app_state.status_message = None;
        drop(app_state);
        trigger_refresh(app_arc, config, tab_index);
    }
}

fn trigger_refresh(app: Arc<Mutex<App>>, config: Config, tab_index: usize) {
    tokio::spawn(async move {
        let items = fetch_data_for_tab(config, tab_index).await.unwrap_or_else(|_| vec![]);
        let mut locked = app.lock().await;
        if locked.current_tab == tab_index {
            locked.items = items;
            locked.is_loading = false;
            if !locked.items.is_empty() { locked.state.select(Some(0)); }
        }
    });
}

// --- 6. UI RENDERING ---

fn ui(frame: &mut Frame, app: &mut App) {
    let color_highlight = Color::Cyan;
    let color_text = Color::White;
    let color_meta = Color::DarkGray;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.size());

    // DYNAMIC HEADER
    let titles: Vec<Line> = app.config.feeds.iter().enumerate().map(|(i, feed)| {
        let title_text = format!(" {}.{} ", i + 1, feed.name);
        if i == app.current_tab {
            Line::from(Span::styled(title_text, Style::default().fg(Color::Black).bg(color_highlight).add_modifier(Modifier::BOLD)))
        } else {
            Line::from(Span::styled(title_text, Style::default().fg(color_meta)))
        }
    }).collect();

    let tabs = Tabs::new(titles)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(color_highlight))
            .title(" ┌─ TermNews ──────────────────────────────┐ ")
            .title_alignment(Alignment::Center));
        
    frame.render_widget(tabs, chunks[0]);

    if app.view_mode == ViewMode::List {
        if app.is_loading {
            let loading = Paragraph::new("⚡ Fetching feeds...")
                .alignment(Alignment::Center)
                .style(Style::default().fg(color_highlight))
                .block(Block::default().borders(Borders::LEFT | Borders::RIGHT).border_type(BorderType::Rounded).border_style(Style::default().fg(color_meta)));
            frame.render_widget(loading, chunks[1]);
        } else {
            let items: Vec<ListItem> = app.items.iter().map(|i| {
                let header = Line::from(vec![Span::styled(format!(" {} ", i.title), Style::default().fg(color_text).add_modifier(Modifier::BOLD))]);
                let sub = Line::from(vec![Span::styled(format!("    via {} ", i.source), Style::default().fg(color_meta).add_modifier(Modifier::ITALIC))]);
                ListItem::new(vec![header, sub, Line::from("")])
            }).collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::LEFT | Borders::RIGHT).border_type(BorderType::Rounded).border_style(Style::default().fg(color_meta)))
                .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
                .highlight_symbol(" █ ");
            frame.render_stateful_widget(list, chunks[1], &mut app.state);
        }
        
        let status_text = if let Some(msg) = &app.status_message { format!(" ✅ {} ", msg) } else { " [1-9] Switch  [Enter] Read  [s] Save  [c] Config  [q] Quit ".to_string() };
        let footer = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Black).bg(color_highlight))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::NONE));
        frame.render_widget(footer, chunks[2]);

    } else {
        let block = Block::default().borders(Borders::ALL).border_type(BorderType::Thick).border_style(Style::default().fg(Color::Green)).title(" 📖 Reading Mode ");
        let paragraph = Paragraph::new(app.article_text.clone()).block(block).style(Style::default().fg(Color::White)).wrap(Wrap { trim: true }).scroll((app.scroll, 0));
        frame.render_widget(paragraph, chunks[1]);
        
        let footer = Paragraph::new(" ▼/▲ Scroll  [q] Back ").style(Style::default().fg(Color::Black).bg(Color::Green)).alignment(Alignment::Center);
        frame.render_widget(footer, chunks[2]);
    }
}