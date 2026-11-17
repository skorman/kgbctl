//! Rendering. Laid out like the Textual POC: header, status line, the overview
//! or detail view, and a footer of key bindings.

use chrono::{Local, Utc};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Cell, Paragraph, Row, Table, Tabs};

use super::app::{App, View};
use super::format::{grouped, money, pct, price_str, signed_money, signed_pct};
use crate::db::SyncStatus;
use crate::portfolio::{Source, Tab, Totals};

const PRIMARY: Color = Color::Blue;
const ZEBRA: Color = Color::Indexed(236);
const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const COLUMNS: [&str; 7] = [
    "Ticker",
    "Value ($)",
    "Shares",
    "Cost",
    "Price",
    "Weight",
    "% P/L",
];
const KEYS: [(&str, &str); 5] = [
    ("q", "Quit"),
    ("r", "Reload"),
    ("s", "Sync"),
    ("o", "Overview / Detail"),
    ("←/→", "Prev / Next ETF"),
];

fn pl_color(x: f64) -> Color {
    if x >= 0.0 { Color::Green } else { Color::Red }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [header, status, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    if let Some(splash) = &mut app.splash {
        splash.render(frame.area(), frame.buffer_mut());
        return;
    }

    draw_header(frame, header);
    draw_status(frame, status, app);
    match app.view {
        View::Overview => draw_overview(frame, body, app),
        View::Detail => draw_detail(frame, body, app),
    }
    draw_footer(frame, footer);
}

fn draw_header(frame: &mut Frame, area: Rect) {
    let style = Style::new().bg(PRIMARY).fg(Color::White);
    frame.render_widget(Block::new().style(style), area);
    frame.render_widget(Line::from("kgbctl").bold().centered(), area);
    let clock = format!("{} ", Local::now().format("%H:%M:%S"));
    frame.render_widget(Line::from(clock).right_aligned(), area);
}

fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    let mut spans = vec![Span::raw("r=reload, s=sync, q=quit")];
    if !app.syncing.is_empty() {
        spans.push(Span::raw("  |  "));
        let target = match &app.remote {
            Some(host) => format!("Syncing brokers -> {host}..."),
            None => "Syncing brokers...".into(),
        };
        spans.push(Span::raw(target).dim());
    } else if let (Some(host), true) = (&app.remote, app.synced_remotely) {
        spans.push(Span::raw("  |  "));
        spans.push(
            Span::raw(format!(
                "Synced to {host}; this view shows the local database"
            ))
            .dim(),
        );
    }
    if let Some(e) = &app.error {
        spans.push(Span::raw("  |  "));
        spans.push(Span::raw(format!("Error: {e}")).red());
    }
    frame.render_widget(Line::from(spans), area);
}

fn draw_footer(frame: &mut Frame, area: Rect) {
    let mut spans = Vec::new();
    for (key, label) in KEYS {
        spans.push(
            Span::raw(format!(" {key} "))
                .bold()
                .fg(Color::White)
                .bg(PRIMARY),
        );
        spans.push(Span::raw(format!(" {label}  ")));
    }
    frame.render_widget(Line::from(spans), area);
}

fn spinner(app: &App) -> Option<char> {
    (!app.syncing.is_empty()).then(|| SPINNER[app.frame % SPINNER.len()])
}

fn draw_overview(frame: &mut Frame, area: Rect, app: &App) {
    let [cards, breakdown] =
        Layout::vertical([Constraint::Length(5), Constraint::Fill(1)]).areas(area);
    draw_cards(frame, cards, "Total AUM", app.snapshot.totals, spinner(app));

    let lines: Vec<Line> = app
        .snapshot
        .sources
        .iter()
        .map(|s| source_line(s, app))
        .collect();
    let inner = breakdown.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    frame.render_widget(Paragraph::new(lines), inner);
}

fn source_line<'a>(s: &Source, app: &App) -> Line<'a> {
    let label = format!("{:<22} ", s.label());
    if app.syncing.contains(&s.broker) {
        let spin = spinner(app).unwrap_or(' ');
        return Line::from(format!("{label}{spin} syncing...")).dim();
    }
    let Some(run) = &s.last_run else {
        return Line::from(format!("{label}unavailable")).red();
    };
    let t = s.totals;
    let mut spans = vec![
        Span::raw(format!("{label}{:>14}   ", money(t.value))),
        Span::raw(format!(
            "{} ({})",
            signed_money(t.pl()),
            signed_pct(t.pl_pct())
        ))
        .fg(pl_color(t.pl())),
    ];
    let when = run.finished_at.unwrap_or(run.started_at);
    let note = match run.status {
        SyncStatus::Ok => format!("   synced {}", ago(when)),
        SyncStatus::Running => format!("   sync running since {}", ago(run.started_at)),
        SyncStatus::Error => format!(
            "   last sync failed {}: {}",
            ago(when),
            run.error.as_deref().unwrap_or("unknown error")
        ),
    };
    let note = Span::raw(note).dim();
    spans.push(if run.status == SyncStatus::Error {
        note.red()
    } else {
        note
    });
    Line::from(spans)
}

fn ago(t: chrono::DateTime<Utc>) -> String {
    let secs = (Utc::now() - t).num_seconds().max(0);
    match secs {
        0..60 => format!("{secs}s ago"),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86400 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}

fn draw_cards(frame: &mut Frame, area: Rect, value_label: &str, t: Totals, spin: Option<char>) {
    let color = pl_color(t.pl());
    let cards = [
        (value_label, money(t.value), Style::new().bold()),
        (
            "Total P/L $",
            signed_money(t.pl()),
            Style::new().bold().fg(color),
        ),
        (
            "Total P/L %",
            signed_pct(t.pl_pct()),
            Style::new().bold().fg(color),
        ),
    ];
    let areas = Layout::horizontal([Constraint::Fill(1); 3]).split(area);
    for ((label, value, style), &area) in cards.into_iter().zip(areas.iter()) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(PRIMARY);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let label = match spin {
            Some(c) => format!("{label} {c}"),
            None => label.to_string(),
        };
        let text = Text::from(vec![Line::from(label).dim(), Line::styled(value, style)]);
        let [middle] = Layout::vertical([Constraint::Length(2)])
            .flex(Flex::Center)
            .areas(inner);
        frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), middle);
    }
}

fn draw_detail(frame: &mut Frame, area: Rect, app: &mut App) {
    let [tabs_area, cards, table] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(5),
        Constraint::Fill(1),
    ])
    .areas(area);

    let titles = app.snapshot.tabs.iter().map(|t| format!(" {} ", t.name));
    let tabs = Tabs::new(titles)
        .select(app.tab)
        .divider(" ")
        .style(Style::new().dim())
        .highlight_style(Style::new().not_dim().bold().underlined().fg(PRIMARY));
    frame.render_widget(tabs, tabs_area);

    let Some(tab) = app.snapshot.tabs.get(app.tab) else {
        return;
    };
    draw_cards(frame, cards, "Total Value", tab.totals, None);
    let widget = holdings_table(tab);
    frame.render_stateful_widget(widget, table, &mut app.tables[app.tab]);
}

fn holdings_table(tab: &Tab) -> Table<'static> {
    let total = if tab.totals.value != 0.0 {
        tab.totals.value
    } else {
        1.0
    };
    let rows: Vec<[String; 7]> = tab
        .holdings
        .iter()
        .map(|h| {
            [
                h.ticker.clone(),
                grouped(h.value, 2),
                grouped(h.shares, 4),
                price_str(h.cost()),
                price_str(h.price),
                pct(h.value / total),
                signed_pct(h.pl_pct()),
            ]
        })
        .collect();

    let widths: Vec<Constraint> = (0..COLUMNS.len())
        .map(|c| {
            let w = rows
                .iter()
                .map(|r| r[c].chars().count())
                .chain([COLUMNS[c].chars().count()])
                .max();
            Constraint::Length(w.unwrap_or(0) as u16)
        })
        .collect();

    let align = |c: usize, s: String| -> Line<'static> {
        if c == 0 {
            Line::from(s)
        } else {
            Line::from(s).right_aligned()
        }
    };
    let header = Row::new(
        COLUMNS
            .iter()
            .enumerate()
            .map(|(c, h)| Cell::from(align(c, h.to_string()))),
    )
    .style(Style::new().bold());

    let body = rows
        .into_iter()
        .zip(&tab.holdings)
        .enumerate()
        .map(|(i, (cells, h))| {
            let pl = h.pl_pct();
            let row = Row::new(cells.into_iter().enumerate().map(|(c, s)| {
                let line = align(c, s);
                Cell::from(if c == 6 { line.fg(pl_color(pl)) } else { line })
            }));
            if i % 2 == 1 {
                row.style(Style::new().bg(ZEBRA))
            } else {
                row
            }
        });

    Table::new(body, widths)
        .header(header)
        .column_spacing(2)
        .row_highlight_style(Style::new().bg(PRIMARY).fg(Color::White).bold())
}
