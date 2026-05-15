use crate::db::{Database, SessionStats};
use crate::proxy::ProxyEvent;
use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
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
                    Constraint::Length(3),  // Header
                    Constraint::Length(8),  // Stats panels
                    Constraint::Min(6),     // Events + Log
                    Constraint::Length(3),  // Footer
                ])
                .split(area);

            render_header(frame, chunks[0], &stats);
            render_stats_panels(frame, chunks[1], &stats, &events);
            render_events_panel(frame, chunks[2], &events);
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

    let header = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan));

    let text = Paragraph::new(Line::from(vec![
        Span::styled(" TokenJ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("│"),
        Span::styled(format!(" 节省 ${:.2}", total_saving), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(format!(" 成本 ${:.2}", total_cost)).fg(Color::Yellow),
        Span::raw(" │ 命中率 "),
        Span::styled(format!("{:.1}%", stats.cache_hit_rate), Style::default().fg(Color::Cyan)),
        Span::raw(format!(" 请求 {}", stats.total_requests)),
    ]))
    .block(header);

    frame.render_widget(text, area);
}

fn render_stats_panels(frame: &mut Frame, area: Rect, stats: &SessionStats, events: &[ProxyEvent]) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    // Panel 1: 节省概览
    let total_cost = stats.total_cost_cents / 100.0;
    let total_saving = stats.total_saving_cents / 100.0;

    let panel1_items = vec![
        ListItem::new(format!("今日节省: ${:.2}", total_saving)).style(Style::default().fg(Color::Green)),
        ListItem::new(format!("今日成本: ${:.2}", total_cost)),
        ListItem::new(format!("节省率: {:.1}%", stats.avg_saving_rate)).style(Style::default().fg(Color::Yellow)),
    ];
    let panel1 = List::new(panel1_items)
        .block(Block::default().borders(Borders::ALL).title("💰 节省概览").fg(Color::Green));
    frame.render_widget(panel1, chunks[0]);

    // Panel 2: 请求统计
    let panel2_items = vec![
        ListItem::new(format!("请求总数: {}", stats.total_requests)),
        ListItem::new(format!("输入 Token: {}", stats.total_input_tokens)),
        ListItem::new(format!("输出 Token: {}", stats.total_output_tokens)),
    ];
    let panel2 = List::new(panel2_items)
        .block(Block::default().borders(Borders::ALL).title("📊 请求统计").fg(Color::Blue));
    frame.render_widget(panel2, chunks[1]);

    // Panel 3: 缓存
    let panel3_items = vec![
        ListItem::new(format!("缓存命中: {} tokens", stats.total_cached_tokens)).style(Style::default().fg(Color::Green)),
        ListItem::new(format!("缓存写入: {} tokens", stats.total_cache_write_tokens)).style(Style::default().fg(Color::Yellow)),
        ListItem::new(format!("命中率: {:.1}%", stats.cache_hit_rate)).style(Style::default().fg(Color::Cyan)),
    ];
    let panel3 = List::new(panel3_items)
        .block(Block::default().borders(Borders::ALL).title("🎯 缓存状态").fg(Color::Cyan));
    frame.render_widget(panel3, chunks[2]);

    // Panel 4: 模型分布
    let model_counts = count_models(events);
    let panel4_items: Vec<ListItem> = if model_counts.is_empty() {
        vec![ListItem::new("等待数据...").style(Style::default().fg(Color::DarkGray))]
    } else {
        let total: usize = model_counts.iter().map(|(_, c)| c).sum();
        model_counts.iter().take(5).map(|(model, count)| {
            let pct = if total > 0 { *count as f64 / total as f64 * 100.0 } else { 0.0 };
            ListItem::new(format!("{} {:.0}%", model, pct))
        }).collect()
    };
    let panel4 = List::new(panel4_items)
        .block(Block::default().borders(Borders::ALL).title("📈 模型分布").fg(Color::Magenta));
    frame.render_widget(panel4, chunks[3]);
}

fn render_events_panel(frame: &mut Frame, area: Rect, events: &[ProxyEvent]) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60),
            Constraint::Percentage(40),
        ])
        .split(area);

    // 上半：实时请求列表
    let items: Vec<ListItem> = events.iter().take(10).map(|e| {
        let saving = e.saving_cents / 100.0;

        let (symbol, style) = if e.cache_write_tokens > 0 {
            ("📝", Style::default().fg(Color::Yellow))
        } else if e.cached_tokens > 0 {
            ("✅", Style::default().fg(Color::Green))
        } else {
            ("  ", Style::default().fg(Color::DarkGray))
        };

        ListItem::new(format!(
            "{} {:<14} in:{:<7} cached:{:<7} ${:.4} ({:.0}%)",
            symbol, e.model, e.input_tokens, e.cached_tokens, saving, e.saving_rate
        ))
        .style(style)
    }).collect();

    let list = List::new(items)
        .block(Block::default()
            .title("📡 实时请求")
            .borders(Borders::ALL)
            .fg(Color::Cyan));

    frame.render_widget(list, chunks[0]);

    // 下半：日志面板
    let log_lines: Vec<Line> = events.iter().take(6).map(|e| {
        let saving = e.saving_cents / 100.0;
        let status = if e.cache_write_tokens > 0 {
            "CACHE_WRITE"
        } else if e.cached_tokens > 0 {
            "HIT"
        } else {
            "MISS"
        };
        Line::from(Span::raw(format!(
            "[{}] {} | {} | in:{} cached:{} ${:.4}",
            status, e.provider, e.model, e.input_tokens, e.cached_tokens, saving
        )))
    }).collect();

    let log_block = Paragraph::new(log_lines)
        .block(Block::default()
            .title("📋 事件日志")
            .borders(Borders::ALL)
            .fg(Color::DarkGray))
        .wrap(Wrap { trim: false });
    frame.render_widget(log_block, chunks[1]);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let text = Paragraph::new(Line::from(vec![
        Span::raw("  [q] 退出  "),
        Span::styled("TokenJ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" 自动缓存优化引擎  "),
        Span::styled("装了就省钱", Style::default().fg(Color::Green)),
    ]))
    .block(Block::default().borders(Borders::ALL));

    frame.render_widget(text, area);
}

fn count_models(events: &[ProxyEvent]) -> Vec<(String, usize)> {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for e in events {
        *counts.entry(e.model.clone()).or_insert(0) += 1;
    }
    let mut result: Vec<_> = counts.into_iter().collect();
    result.sort_by(|a, b| b.1.cmp(&a.1));
    result
}
