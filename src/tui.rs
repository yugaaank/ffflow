use std::io;
use std::sync::mpsc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Terminal;

use crate::cli::{self, Commands};
use crate::core;
use crate::core::error::FfxError;
use crate::core::job::{Job, JobStatus};
use crate::core::progress::ProgressUpdate;

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self, FfxError> {
        enable_raw_mode().map_err(|e| FfxError::InvalidCommand {
            message: e.to_string(),
        })?;
        let mut stdout = io::stdout();
        stdout
            .execute(EnterAlternateScreen)
            .map_err(|e| FfxError::InvalidCommand {
                message: e.to_string(),
            })?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = stdout.execute(LeaveAlternateScreen);
    }
}

#[derive(Debug)]
struct AppState {
    input: String,
    history: Vec<String>,
    progress: Option<ProgressUpdate>,
    job_status: Option<JobStatus>,
    last_error: Option<String>,
    should_quit: bool,
    job_running: bool,
    scroll_offset: usize,
    view_lines: usize,
    tick: u64,
    duration_seconds: Option<f64>,
    last_progress_line: Option<String>,
    progress_log_counter: u64,
}

const DIVIDER_MARKER: &str = "<divider>";

impl AppState {
    fn new() -> Self {
        let mut history = Vec::new();
        history.push("Welcome to ffx. Type 'help' for commands.".to_string());
        Self {
            input: String::new(),
            history,
            progress: None,
            job_status: None,
            last_error: None,
            should_quit: false,
            job_running: false,
            scroll_offset: 0,
            view_lines: 1,
            tick: 0,
            duration_seconds: None,
            last_progress_line: None,
            progress_log_counter: 0,
        }
    }

    fn push_history(&mut self, line: impl Into<String>) {
        const MAX_LINES: usize = 500;
        if self.history.len() >= MAX_LINES {
            let drain_count = self.history.len().saturating_sub(MAX_LINES - 1);
            self.history.drain(0..drain_count);
        }
        self.history.push(line.into());
        self.clamp_scroll();
    }

    fn update_job(&mut self, result: Result<Job, FfxError>) {
        self.job_running = false;
        match result {
            Ok(job) => {
                self.job_status = Some(job.status);
                self.push_history(format!("Job {} finished: {:?}", job.id, job.status));
            }
            Err(err) => {
                self.job_status = Some(JobStatus::Failed);
                self.last_error = Some(err.to_string());
                self.push_history(format!("error: {err}"));
            }
        }
    }

    fn set_view_lines(&mut self, lines: usize) {
        self.view_lines = lines.max(1);
        self.clamp_scroll();
    }

    fn scroll_up(&mut self, lines: usize) {
        let max_scroll = self.max_scroll();
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
    }

    fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    fn scroll_top(&mut self) {
        self.scroll_offset = self.max_scroll();
    }

    fn scroll_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    fn max_scroll(&self) -> usize {
        self.history.len().saturating_sub(self.view_lines)
    }

    fn clamp_scroll(&mut self) {
        let max_scroll = self.max_scroll();
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }
    }
}

pub fn run() -> Result<(), FfxError> {
    let _guard = TerminalGuard::enter()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| FfxError::InvalidCommand {
        message: e.to_string(),
    })?;

    let (progress_tx, progress_rx) = mpsc::channel::<ProgressUpdate>();
    let (log_tx, log_rx) = mpsc::channel::<String>();
    let (job_tx, job_rx) = mpsc::channel::<Result<Job, FfxError>>();

    let mut app = AppState::new();

    loop {
        while let Ok(update) = progress_rx.try_recv() {
            app.progress = Some(update.clone());
            if let Some(line) = format_progress_line(&update) {
                app.last_progress_line = Some(line.clone());
                app.progress_log_counter = app.progress_log_counter.wrapping_add(1);
                if app.progress_log_counter % 25 == 0 {
                    app.push_history(line);
                }
            }
        }

        while let Ok(line) = log_rx.try_recv() {
            if core::progress::parse_progress_line(&line).is_none() && should_log_line(&line) {
                app.push_history(line);
            }
        }

        while let Ok(result) = job_rx.try_recv() {
            app.update_job(result);
        }

        let size = terminal.size().map_err(|e| FfxError::InvalidCommand {
            message: e.to_string(),
        })?;
        let history_height = size.height.saturating_sub(7).max(3) as usize;
        let view_lines = history_height.saturating_sub(2).max(1);
        app.set_view_lines(view_lines);

        app.tick = app.tick.wrapping_add(1);

        terminal
            .draw(|frame| {
                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(4),
                        Constraint::Min(3),
                        Constraint::Length(3),
                    ])
                    .split(frame.size());

                let header = render_header(&app, layout[0].width as usize);
                frame.render_widget(header, layout[0]);

                let history = render_history(&app, layout[1].height as usize, layout[1].width as usize);
                frame.render_widget(history, layout[1]);

                let input = Paragraph::new(app.input.as_str())
                    .block(Block::default().title("Input").borders(Borders::ALL))
                    .wrap(Wrap { trim: false });
                frame.render_widget(input, layout[2]);
                frame.set_cursor(
                    layout[2].x + 1 + app.input.len() as u16,
                    layout[2].y + 1,
                );
            })
            .map_err(|e| FfxError::InvalidCommand {
                message: e.to_string(),
            })?;

        if event::poll(Duration::from_millis(50)).map_err(|e| FfxError::InvalidCommand {
            message: e.to_string(),
        })? {
            if let Event::Key(key) = event::read().map_err(|e| FfxError::InvalidCommand {
                message: e.to_string(),
            })? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true;
                    }
                    KeyCode::Char(ch) => {
                        app.input.push(ch);
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Enter => {
                        let line = app.input.trim().to_string();
                        app.input.clear();
                        if !line.is_empty() {
                            handle_line(&mut app, line, progress_tx.clone(), log_tx.clone(), job_tx.clone());
                        }
                    }
                    KeyCode::PageUp => {
                        let step = app.view_lines.saturating_sub(1).max(1);
                        app.scroll_up(step);
                    }
                    KeyCode::PageDown => {
                        let step = app.view_lines.saturating_sub(1).max(1);
                        app.scroll_down(step);
                    }
                    KeyCode::Up => {
                        app.scroll_up(1);
                    }
                    KeyCode::Down => {
                        app.scroll_down(1);
                    }
                    KeyCode::Home => {
                        app.scroll_top();
                    }
                    KeyCode::End => {
                        app.scroll_bottom();
                    }
                    KeyCode::Esc => {
                        app.should_quit = true;
                    }
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn handle_line(
    app: &mut AppState,
    line: String,
    progress_tx: mpsc::Sender<ProgressUpdate>,
    log_tx: mpsc::Sender<String>,
    job_tx: mpsc::Sender<Result<Job, FfxError>>,
) {
    let trimmed = line.trim();
    if !app.history.is_empty() {
        app.push_history(DIVIDER_MARKER);
    }
    app.push_history(format!(">> {trimmed}"));

    if trimmed.eq_ignore_ascii_case("quit") || trimmed.eq_ignore_ascii_case("exit") {
        app.should_quit = true;
        return;
    }

    if trimmed.eq_ignore_ascii_case("clear") {
        app.history.clear();
        app.scroll_bottom();
        return;
    }

    if trimmed.eq_ignore_ascii_case("help") {
        app.push_history("Commands:".to_string());
        app.push_history("  encode -i <input> -o <output> [--vcodec ...] [--acodec ...] [--preset ...]".to_string());
        app.push_history("  probe -i <input>".to_string());
        app.push_history("  presets".to_string());
        app.push_history("  ffmpeg <args...>".to_string());
        app.push_history("  clear / exit".to_string());
        return;
    }

    if trimmed.eq_ignore_ascii_case("presets") {
        for preset in cli::PRESETS {
            app.push_history(preset);
        }
        return;
    }

    if app.job_running {
        app.push_history("A job is already running. Please wait for it to finish.".to_string());
        return;
    }

    if let Some(rest) = trimmed.strip_prefix("ffmpeg ") {
        match shell_words::split(rest) {
            Ok(args) => {
                if args.is_empty() {
                    app.push_history("error: ffmpeg requires arguments".to_string());
                    return;
                }
                app.duration_seconds = parse_duration_from_args(&args);
                app.job_running = true;
                app.job_status = Some(JobStatus::Running);
                app.progress = None;
                app.last_progress_line = None;
                std::thread::spawn(move || {
                    let result = core::run_args_with_progress(args, progress_tx, Some(log_tx));
                    let _ = job_tx.send(result);
                });
            }
            Err(err) => {
                app.push_history(format!("error: {err}"));
            }
        }
        return;
    }

    match cli::parse_line(trimmed) {
        Ok(Commands::Encode(args)) => {
            let cmd = cli::encode_args_to_command(args);
            app.duration_seconds = parse_duration_from_args(&cmd.extra_args);
            app.job_running = true;
            app.job_status = Some(JobStatus::Running);
            app.progress = None;
            app.last_progress_line = None;
            std::thread::spawn(move || {
                let result = core::run_with_progress(cmd, progress_tx, Some(log_tx));
                let _ = job_tx.send(result);
            });
        }
        Ok(Commands::Probe(args)) => {
            let cmd = cli::probe_args_to_command(args);
            app.duration_seconds = parse_duration_from_args(&cmd.extra_args);
            app.job_running = true;
            app.job_status = Some(JobStatus::Running);
            app.progress = None;
            app.last_progress_line = None;
            std::thread::spawn(move || {
                let result = core::run_with_progress(cmd, progress_tx, Some(log_tx));
                let _ = job_tx.send(result);
            });
        }
        Ok(Commands::Presets) => {
            for preset in cli::PRESETS {
                app.push_history(preset);
            }
        }
        Err(err) => {
            app.push_history(format!("error: {err}"));
        }
    }
}

fn render_header(app: &AppState, width: usize) -> Paragraph<'static> {
    let status = match app.job_status {
        Some(JobStatus::Pending) => "Pending",
        Some(JobStatus::Running) => "Running",
        Some(JobStatus::Finished) => "Finished",
        Some(JobStatus::Failed) => "Failed",
        None => "Idle",
    };

    let progress = match &app.progress {
        Some(update) => format!(
            "time={} frame={} speed={}",
            update.time.clone().unwrap_or_default(),
            update.frame.map(|v| v.to_string()).unwrap_or_default(),
            update.speed.clone().unwrap_or_default()
        ),
        None => "time= frame= speed=".to_string(),
    };

    let bar_width = width.saturating_sub(30).clamp(10, 40);
    let progress_bar = render_progress_bar(app, bar_width);

    let text = vec![
        Line::from(vec![Span::raw("Status: "), Span::raw(status)]),
        Line::from(vec![
            Span::raw(progress_bar),
            Span::raw(" "),
            Span::raw(progress),
        ]),
    ];

    Paragraph::new(text)
        .block(Block::default().title("ffx").borders(Borders::ALL))
        .wrap(Wrap { trim: true })
}

fn render_progress_bar(app: &AppState, width: usize) -> String {
    let width = width.max(10);
    let mut bar = String::with_capacity(width + 2);
    bar.push('[');

    if !app.job_running {
        for _ in 0..width {
            bar.push(' ');
        }
        bar.push(']');
        return bar;
    }

    if let (Some(update), Some(total)) = (&app.progress, app.duration_seconds) {
        if let Some(elapsed) = update.time.as_deref().and_then(parse_ffmpeg_time) {
            let ratio = (elapsed / total).clamp(0.0, 1.0);
            let filled = ((ratio * width as f64).round() as usize).min(width);
            for idx in 0..width {
                if idx < filled {
                    bar.push('=');
                } else if idx == filled && filled < width {
                    bar.push('>');
                } else {
                    bar.push(' ');
                }
            }
            bar.push(']');
            return bar;
        }
    }

    let pos = (app.tick as usize) % width;
    for idx in 0..width {
        if idx == pos {
            bar.push('>');
        } else if idx < pos {
            bar.push('=');
        } else {
            bar.push(' ');
        }
    }
    bar.push(']');
    bar
}

fn render_history(app: &AppState, height: usize, width: usize) -> Paragraph<'static> {
    let max_lines = height.saturating_sub(2).max(1);
    let end = app.history.len().saturating_sub(app.scroll_offset);
    let start = end.saturating_sub(max_lines);
    let divider_width = width.saturating_sub(2).max(1);
    let divider = "─".repeat(divider_width);
    let lines: Vec<Line> = app.history[start..end]
        .iter()
        .map(|line| {
            if line == DIVIDER_MARKER {
                Line::from(Span::raw(divider.clone()))
            } else {
                Line::from(line.clone())
            }
        })
        .collect();

    Paragraph::new(lines)
        .block(Block::default().title("Session").borders(Borders::ALL))
        .wrap(Wrap { trim: false })
}

fn should_log_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    let prefixes = [
        "ffmpeg version",
        "built with",
        "configuration:",
        "libavutil",
        "libavcodec",
        "libavformat",
        "libavdevice",
        "libavfilter",
        "libswscale",
        "libswresample",
    ];

    for prefix in prefixes {
        if trimmed.starts_with(prefix) {
            return false;
        }
    }

    true
}

fn parse_duration_from_args(args: &[String]) -> Option<f64> {
    let mut idx = 0;
    while idx < args.len() {
        if args[idx] == "-t" {
            if let Some(value) = args.get(idx + 1) {
                if let Ok(seconds) = value.parse::<f64>() {
                    return Some(seconds);
                }
                if let Some(seconds) = parse_ffmpeg_time(value) {
                    return Some(seconds);
                }
            }
        }
        if let Some(pos) = args[idx].find("duration=") {
            let value = &args[idx][pos + "duration=".len()..];
            let value = value.split(':').next().unwrap_or(value);
            if let Ok(seconds) = value.parse::<f64>() {
                return Some(seconds);
            }
        }
        idx += 1;
    }
    None
}

fn parse_ffmpeg_time(value: &str) -> Option<f64> {
    let mut parts = value.split(':').collect::<Vec<_>>();
    if parts.len() == 1 {
        return parts[0].parse::<f64>().ok();
    }
    if parts.len() == 2 {
        let minutes = parts[0].parse::<f64>().ok()?;
        let seconds = parts[1].parse::<f64>().ok()?;
        return Some(minutes * 60.0 + seconds);
    }
    if parts.len() == 3 {
        let hours = parts[0].parse::<f64>().ok()?;
        let minutes = parts[1].parse::<f64>().ok()?;
        let seconds = parts[2].parse::<f64>().ok()?;
        return Some(hours * 3600.0 + minutes * 60.0 + seconds);
    }
    None
}

fn format_progress_line(update: &ProgressUpdate) -> Option<String> {
    let time = update.time.clone().unwrap_or_default();
    let frame = update.frame.map(|v| v.to_string()).unwrap_or_default();
    let speed = update.speed.clone().unwrap_or_default();
    if time.is_empty() && frame.is_empty() && speed.is_empty() {
        None
    } else {
        Some(format!("progress: time={time} frame={frame} speed={speed}"))
    }
}
