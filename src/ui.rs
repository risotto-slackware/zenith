use crate::SystemSnapshot;
use crate::system::get_process_details;
use ratatui::layout::Rect;
use crossterm::event::{MouseEventKind, MouseButton, MouseEvent};
use std::time::{Instant, Duration};
use anyhow::Result;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Gauge, Row, Table, Paragraph, Wrap};
use ratatui::text::{Span, Spans};
use ratatui::widgets::TableState;
use libc;
use ratatui::Terminal;
use std::io::Stdout;
use std::sync::Arc;
use tokio::sync::watch::Receiver;

pub async fn run_ui(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    rx: Arc<tokio::sync::Mutex<Receiver<SystemSnapshot>>>,
) -> Result<()> {
    let mut show_details = false;
    let mut show_proc_details = false;
    let mut table_state = TableState::default();
    let mut selected_idx: usize = 0;
    table_state.select(Some(0));
    let mut last_table_area: Option<Rect> = None;
    let mut last_click: Option<(Instant, u16, u16)> = None; // time, col, row
    let mut status_msg: Option<String> = None;
    let mut status_expire: Option<Instant> = None;

    loop {
        // receive latest snapshot (non-blocking)
        let snap = {
            let guard = rx.lock().await;
            let snapshot = guard.borrow().clone();
            snapshot
        };

            terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(3),
                    Constraint::Length(1),
                ])
                .split(size);

            // CPU gauge
            let g = Gauge::default()
                .block(Block::default().borders(Borders::ALL).title("CPU"))
                .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Reset))
                .ratio(snap.cpu_usage.max(0.0).min(1.0));
            f.render_widget(g, chunks[0]);

            // Memory gauge
            let used_kb = snap.mem_total_kb.saturating_sub(snap.mem_available_kb);
            let mem_ratio = if snap.mem_total_kb > 0 {
                used_kb as f64 / snap.mem_total_kb as f64
            } else {
                0.0
            };
            let mem_title = format!("Memory: {} / {} MB", used_kb / 1024, snap.mem_total_kb / 1024);
            let mg = Gauge::default()
                .block(Block::default().borders(Borders::ALL).title(mem_title))
                .gauge_style(Style::default().fg(Color::Green))
                .ratio(mem_ratio);
            f.render_widget(mg, chunks[1]);

            // Processes and optional details
                if show_details {
                let bottom_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(chunks[2]);

                let rows: Vec<Row> = snap
                    .processes
                    .iter()
                    .map(|p| Row::new(vec![p.pid.to_string(), p.rss_kb.to_string(), p.cmd.clone()]))
                    .collect();

                let table = Table::new(rows)
                    .header(
                        Row::new(vec!["PID", "RSS(kB)", "Command"]).style(
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                        ),
                    )
                    .block(Block::default().borders(Borders::ALL).title("Top Processes"))
                    .widths(&[
                        Constraint::Length(8),
                        Constraint::Length(12),
                        Constraint::Min(10),
                    ])
                    .column_spacing(1);

                // ensure selected index is valid
                if !snap.processes.is_empty() {
                    if selected_idx >= snap.processes.len() {
                        selected_idx = snap.processes.len() - 1;
                    }
                    table_state.select(Some(selected_idx));
                } else {
                    table_state.select(None);
                }

                f.render_stateful_widget(table, bottom_chunks[0], &mut table_state);
                last_table_area = Some(bottom_chunks[0]);

                // Details pane shows either per-core gauges or selected process details
                if show_proc_details {
                    // show selected process details
                    let det = snap.processes.get(selected_idx).and_then(|p| get_process_details(p.pid).ok());
                    let mut text = vec![];
                    if let Some(det) = det {
                        text.push(Spans::from(Span::styled(format!("PID: {}", det.pid), Style::default().add_modifier(Modifier::BOLD))));
                        if !det.cmdline.is_empty() { text.push(Spans::from(Span::raw(format!("Cmd: {}", det.cmdline)))); }
                        if !det.exe.is_empty() { text.push(Spans::from(Span::raw(format!("Exe: {}", det.exe)))); }
                        if !det.cwd.is_empty() { text.push(Spans::from(Span::raw(format!("Cwd: {}", det.cwd)))); }
                        if let Some(uid) = det.uid { text.push(Spans::from(Span::raw(format!("UID: {}", uid)))); }
                        if let Some(gid) = det.gid { text.push(Spans::from(Span::raw(format!("GID: {}", gid)))); }
                        if let Some(threads) = det.threads { text.push(Spans::from(Span::raw(format!("Threads: {}", threads)))); }
                        if let Some(rss) = det.rss_kb { text.push(Spans::from(Span::raw(format!("RSS: {} KB", rss)))); }
                        if let Some(fds) = det.open_fds { text.push(Spans::from(Span::raw(format!("Open fds: {}", fds)))); }
                    } else {
                        text.push(Spans::from(Span::raw("(process details unavailable)")));
                    }
                    let paragraph = Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Process Details")).wrap(Wrap{trim:true});
                    f.render_widget(paragraph, bottom_chunks[1]);
                } else {
                    // existing per-core gauges view
                    let header = Paragraph::new(Spans::from(Span::styled(
                        format!("Loadavg: {}", snap.load_avg),
                        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                    )))
                    .block(Block::default().borders(Borders::BOTTOM));
                    let max_cores = 16usize;
                    let total_entries = snap.per_cpu_usage.len();
                    let show_count = std::cmp::min(total_entries, max_cores);
                    let mut constraints = Vec::with_capacity(show_count + 1);
                    constraints.push(Constraint::Length(1));
                    for _ in 0..show_count { constraints.push(Constraint::Length(1)); }
                    let vchunks = Layout::default().direction(Direction::Vertical).constraints(constraints).split(bottom_chunks[1]);
                    f.render_widget(header, vchunks[0]);
                    fn usage_color(u: f64) -> Color { if u < 0.5 { Color::Green } else if u < 0.8 { Color::Yellow } else { Color::Red } }
                    for i in 0..show_count {
                        if let Some(u) = snap.per_cpu_usage.get(i) {
                            let label = if i == 0 { "all".to_string() } else { format!("cpu{}", i - 1) };
                            let g = Gauge::default().block(Block::default().borders(Borders::NONE).title(label)).gauge_style(Style::default().fg(usage_color(*u))).ratio(u.max(0.0).min(1.0));
                            let idx = 1 + i;
                            if idx < vchunks.len() { f.render_widget(g, vchunks[idx]); }
                        }
                    }
                    last_table_area = Some(bottom_chunks[0]);
                }
            } else {
                let rows: Vec<Row> = snap
                    .processes
                    .iter()
                    .map(|p| Row::new(vec![p.pid.to_string(), p.rss_kb.to_string(), p.cmd.clone()]))
                    .collect();

                let table = Table::new(rows)
                    .header(
                        Row::new(vec!["PID", "RSS(kB)", "Command"]).style(
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                        ),
                    )
                    .block(Block::default().borders(Borders::ALL).title("Top Processes"))
                    .widths(&[
                        Constraint::Length(8),
                        Constraint::Length(12),
                        Constraint::Min(10),
                    ])
                    .column_spacing(1);

                // non-details view: render table with selection
                if !snap.processes.is_empty() {
                    if selected_idx >= snap.processes.len() { selected_idx = snap.processes.len() - 1; }
                    table_state.select(Some(selected_idx));
                } else { table_state.select(None); }
                f.render_stateful_widget(table, chunks[2], &mut table_state);
                last_table_area = Some(chunks[2]);
            }

            // Footer with keybinds or transient status
            // clear expired status
            if let Some(exp) = status_expire {
                if Instant::now() > exp {
                    status_msg = None;
                    status_expire = None;
                }
            }

            let footer_spans = if let Some(msg) = &status_msg {
                vec![Span::styled(msg.clone(), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))]
            } else {
                vec![
                    Span::raw("q: quit  "),
                    Span::raw("d / F4: toggle details  "),
                    Span::raw("Enter: details  k/K: kill"),
                ]
            };

            let footer = Paragraph::new(Spans::from(footer_spans)).style(Style::default().fg(Color::White)).block(Block::default().borders(Borders::TOP));
            f.render_widget(footer, chunks[3]);
        })?;

        // handle input (non-blocking poll)
        use crossterm::event::{poll, read, Event, KeyCode};
        if poll(std::time::Duration::from_millis(100))? {
            let ev = read()?;
            match ev {
                Event::Key(k) => match k.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => break,
                    KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::F(4) => {
                        show_details = !show_details;
                    }
                    KeyCode::Char('\n') | KeyCode::Enter => {
                        // toggle process detail inspect
                        show_proc_details = !show_proc_details;
                    }
                    KeyCode::Up => {
                        if selected_idx > 0 { selected_idx -= 1; }
                    }
                    KeyCode::Down => {
                        if selected_idx + 1 < snap.processes.len() { selected_idx += 1; }
                    }
                    KeyCode::Char('k') => {
                        // SIGTERM
                        if let Some(p) = snap.processes.get(selected_idx) {
                            let ret = unsafe { libc::kill(p.pid, libc::SIGTERM) };
                            if ret == 0 {
                                status_msg = Some(format!("SIGTERM sent to PID {}", p.pid));
                            } else {
                                status_msg = Some(format!("SIGTERM failed: {}", std::io::Error::last_os_error()));
                            }
                            status_expire = Some(Instant::now() + Duration::from_secs(2));
                        }
                    }
                    KeyCode::Char('K') => {
                        // SIGKILL
                        if let Some(p) = snap.processes.get(selected_idx) {
                            let ret = unsafe { libc::kill(p.pid, libc::SIGKILL) };
                            if ret == 0 {
                                status_msg = Some(format!("SIGKILL sent to PID {}", p.pid));
                            } else {
                                status_msg = Some(format!("SIGKILL failed: {}", std::io::Error::last_os_error()));
                            }
                            status_expire = Some(Instant::now() + Duration::from_secs(2));
                        }
                    }
                    _ => {}
                },
                Event::Mouse(me) => {
                    // only handle left-button down
                    if let MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column, row, .. } = me {
                        if let Some(area) = last_table_area {
                            // approximate header offset: border + header => 2 lines
                            let rel_row = row.saturating_sub(area.y + 2);
                            // compute index
                            let idx = rel_row as usize;
                            if idx < snap.processes.len() {
                                selected_idx = idx;
                                table_state.select(Some(selected_idx));
                                let now = Instant::now();
                                if let Some((t, ccol, crow)) = last_click {
                                    if now.duration_since(t).as_millis() <= 300 && ccol == column && crow == row {
                                        // double click: toggle process details
                                        show_proc_details = !show_proc_details;
                                        last_click = None;
                                    } else {
                                        last_click = Some((now, column, row));
                                    }
                                } else {
                                    last_click = Some((now, column, row));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}
