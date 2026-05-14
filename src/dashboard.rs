use crate::db::{Database, SessionStats};
use crate::proxy::ProxyEvent;
use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

pub async fn run_dashboard(
    db: Arc<Database>,
    mut event_rx: broadcast::Receiver<ProxyEvent>,
    running: Arc<AtomicBool>,
) -> Result<()> {
    let mut terminal = ratatui::init();
    let mut events: Vec<ProxyEvent> = Vec::with_capacity(100);
    let mut stats = SessionStats::default();

    while running.load(Ordering::Relaxed) {
        while let Ok(event) = event_rx.try_recv() {
            events.insert(0, event);
            if events.len() > 100 {
                events.pop();
            }
        }
        if let Ok(s) = db.get_stats_since("1970-01-01") {
            stats = s;
        }

        terminal.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(6),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(area);

            render_header(frame, chunks[0], &stats);
            render_stats(frame, chunks[1], &stats);
            render_events(frame, chunks[2], &events);
            render_footer(frame, chunks[3]);
        })?;

        if event::poll(Duration::from_secs(1))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    ratatui::restore();
    Ok(())
}

fn render_header(frame: &mut Frame, area: Rect, stats: &SessionStats) {
    let total_cost = stats.total_cost_cents / 100.0;
    let total_saving = stats.total_saving_cents / 100.0;

    let text = vec![
        Line::from(vec![
            Span::styled(" tokenJ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(format!("已节省: ${:.2}", total_saving), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("  |  累计成本: "),
            Span::styled(format!("${:.2}", total_cost), Style::default().fg(Color::Yellow)),
            Span::raw("  |  缓存命中率: "),
            Span::styled(format!("{:.1}%", stats.cache_hit_rate), Style::default().fg(Color::Cyan)),
        ]),
    ];

    let block = Block::default().borders(Borders::ALL);
    let para = Paragraph::new(text).block(block);
    frame.render_widget(para, area);
}

fn render_stats(frame: &mut Frame, area: Rect, stats: &SessionStats) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25); 4])
        .split(area);

    let total_cost = stats.total_cost_cents / 100.0;
    let total_saving = stats.total_saving_cents / 100.0;

    let list1 = List::new(vec![
        ListItem::new(format!("请求总数: {}", stats.total_requests)),
        ListItem::new(format!("今天成本: ${:.2}", total_cost)),
    ]).block(Block::default().title("概览").borders(Borders::ALL));
    frame.render_widget(list1, chunks[0]);

    let list2 = List::new(vec![
        ListItem::new(format!("节省金额: ${:.2}", total_saving)),
        ListItem::new(format!("节省率: {:.1}%", stats.avg_saving_rate)),
    ]).block(Block::default().title("节省").borders(Borders::ALL));
    frame.render_widget(list2, chunks[1]);

    let list3 = List::new(vec![
        ListItem::new(format!("缓存 Token: {}", stats.total_cached_tokens)),
        ListItem::new(format!("写入 Token: {}", stats.total_cache_write_tokens)),
    ]).block(Block::default().title("缓存").borders(Borders::ALL));
    frame.render_widget(list3, chunks[2]);

    let list4 = List::new(vec![
        ListItem::new(format!("输入 Token: {}", stats.total_input_tokens)),
        ListItem::new(format!("输出 Token: {}", stats.total_output_tokens)),
    ]).block(Block::default().title("Token").borders(Borders::ALL));
    frame.render_widget(list4, chunks[3]);
}

fn render_events(frame: &mut Frame, area: Rect, events: &[ProxyEvent]) {
    let items: Vec<ListItem> = events.iter().take(10).map(|e| {
        let saving = e.saving_cents / 100.0;
        let symbol = if e.cache_write_tokens > 0 { "W" }
                     else if e.cached_tokens > 0 { "H" }
                     else { " " };
        ListItem::new(format!(
            "[{}] {:<12} in:{:<6} cached:{:<6} save:${:.4} ({:.1}%)",
            symbol, e.model, e.input_tokens, e.cached_tokens, saving, e.saving_rate
        ))
    }).collect();

    let list = List::new(items)
        .block(Block::default().title("实时请求").borders(Borders::ALL));
    frame.render_widget(list, area);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(vec![
            Span::raw("按 q 退出  |  tokenJ 自动缓存优化引擎  |  "),
            Span::styled("装了就省钱", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
    ];
    let para = Paragraph::new(text).block(Block::default().borders(Borders::ALL));
    frame.render_widget(para, area);
}
