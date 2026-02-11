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
use crate::core::event::FfmpegEvent;
use crate::core::formatter::{
    format_duration, format_input_line, format_output_line, format_progress_line,
    format_summary_line,
};
use crate::core::job::JobStatus;
use crate::core::metadata::{InputInfo, OutputInfo};
use crate::core::progress::{parse_ffmpeg_time, FfmpegProgress};
use crate::core::summary::EncodeSummary;

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
    progress: Option<FfmpegProgress>,
    input_info: Option<InputInfo>,
    output_info: Option<OutputInfo>,
    summary: Option<EncodeSummary>,
    job_status: Option<JobStatus>,
    last_error: Option<String>,
    should_quit: bool,
    job_running: bool,
    scroll_offset: usize,
    view_lines: usize,
    tick: u64,
    duration: Option<Duration>,
    last_progress_line: Option<String>,
    progress_log_counter: u64,
    stdin_tx: Option<mpsc::Sender<String>>,
    job_queue: std::collections::VecDeque<String>,
}

const DIVIDER_MARKER: &str = "<divider>";

impl AppState {
    fn new(queue: Vec<String>) -> Self {
        let mut history = Vec::new();
        history.push("Welcome to ffx. Type 'help' for commands.".to_string());
        if !queue.is_empty() {
            history.push(format!("Loaded {} jobs from batch file.", queue.len()));
        }
        Self {
            input: String::new(),
            history,
            progress: None,
            input_info: None,
            output_info: None,
            summary: None,
            job_status: None,
            last_error: None,
            should_quit: false,
            job_running: false,
            scroll_offset: 0,
            view_lines: 1,
            tick: 0,
            duration: None,
            last_progress_line: None,
            progress_log_counter: 0,
            stdin_tx: None,
            job_queue: std::collections::VecDeque::from(queue),
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

    fn update_job(&mut self, status: JobStatus) {
        self.job_running = false;
        self.job_status = Some(status);
        self.stdin_tx = None;
        self.push_history(format!("Job finished: {status:?}"));
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

pub fn run(initial_queue: Vec<String>) -> Result<(), FfxError> {
    let _guard = TerminalGuard::enter()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| FfxError::InvalidCommand {
        message: e.to_string(),
    })?;

    let (event_tx, event_rx) = mpsc::channel::<FfmpegEvent>();
    let (job_tx, job_rx) = mpsc::channel::<JobStatus>();

    let mut app = AppState::new(initial_queue);

    loop {
        while let Ok(event) = event_rx.try_recv() {
            match event {
                FfmpegEvent::Progress(update) => {
                    app.progress = Some(update.clone());
                    if let Some(line) = format_progress_line(&update, app.duration) {
                        app.last_progress_line = Some(line.clone());
                        app.progress_log_counter = app.progress_log_counter.wrapping_add(1);
                        if app.progress_log_counter % 25 == 0 {
                            app.push_history(line);
                        }
                    }
                }
                FfmpegEvent::Input(info) => {
                    app.input_info = Some(info.clone());
                    if let Some(duration) = info.duration {
                        app.duration = Some(duration);
                    }
                    app.push_history(format_input_line(&info));
                }
                FfmpegEvent::Output(info) => {
                    app.output_info = Some(info.clone());
                    app.push_history(format_output_line(&info));
                }
                FfmpegEvent::Summary(summary) => {
                    app.summary = Some(summary.clone());
                    app.push_history(format_summary_line(&summary));
                }
                FfmpegEvent::Error(message) => {
                    app.last_error = Some(message.clone());
                    app.job_status = Some(JobStatus::Failed);
                    app.push_history(format!("error: {message}"));
                }
                FfmpegEvent::Prompt(message) => {
                    app.job_status = Some(JobStatus::AwaitingConfirmation);
                    app.push_history(format!("PROMPT: {message}"));
                    app.push_history(">> Press 'y' to confirm or 'n' to abort.");
                }
            }
        }

        while let Ok(status) = job_rx.try_recv() {
            app.update_job(status);
        }

        if !app.job_running && app.job_status != Some(JobStatus::AwaitingConfirmation) {
            if let Some(next_cmd) = app.job_queue.pop_front() {
                handle_line(&mut app, next_cmd, event_tx.clone(), job_tx.clone());
            }
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

                let input_text = if app.job_status == Some(JobStatus::AwaitingConfirmation) {
                    format!("{} (y/n)", app.input)
                } else {
                    app.input.clone()
                };

                let input = Paragraph::new(input_text.as_str())
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
                if let Some(JobStatus::AwaitingConfirmation) = app.job_status {
                    match key.code {
                         KeyCode::Char('y') | KeyCode::Char('Y') => {
                            if let Some(tx) = &app.stdin_tx {
                                let _ = tx.send("y\n".to_string());
                            }
                            app.job_status = Some(JobStatus::Running);
                            app.push_history(">> Sent: y");
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            if let Some(tx) = &app.stdin_tx {
                                let _ = tx.send("n\n".to_string());
                            }
                            app.job_status = Some(JobStatus::Running);
                             app.push_history(">> Sent: n");
                        }
                        KeyCode::Esc => {
                            app.should_quit = true;
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.should_quit = true;
                        }
                        _ => {}
                    }
                } else {
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
                                handle_line(&mut app, line, event_tx.clone(), job_tx.clone());
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
    event_tx: mpsc::Sender<FfmpegEvent>,
    job_tx: mpsc::Sender<JobStatus>,
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
        app.push_history("  presets".to_string());
        app.push_history("  ffmpeg <args...>".to_string());
        app.push_history("  batch <file.flw>".to_string());
        app.push_history("  clear / exit".to_string());
        return;
    }

    if let Some(path_str) = trimmed.strip_prefix("batch ") {
        let path = std::path::Path::new(path_str.trim());
        match core::batch::parse_flw_file(path) {
            Ok(commands) => {
                let count = commands.len();
                app.job_queue.extend(commands);
                app.push_history(format!("Loaded {} jobs from '{}'.", count, path.display()));
            }
            Err(e) => {
                app.push_history(format!("error reading batch file: {}", e));
            }
        }
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
                app.duration = parse_duration_from_args(&args);
                app.job_running = true;
                app.job_status = Some(JobStatus::Running);
                app.progress = None;
                app.last_progress_line = None;
                app.last_error = None;

                let (rx, tx) = core::runner::run_args_with_events(args);
                app.stdin_tx = Some(tx);

                std::thread::spawn(move || {
                    let mut had_error = false;
                    for event in rx {
                        if matches!(event, FfmpegEvent::Error(_)) {
                            had_error = true;
                        }
                        let _ = event_tx.send(event);
                    }
                    let status = if had_error {
                        JobStatus::Failed
                    } else {
                        JobStatus::Finished
                    };
                    let _ = job_tx.send(status);
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
            app.duration = parse_duration_from_args(&cmd.extra_args);
            app.job_running = true;
            app.job_status = Some(JobStatus::Running);
            app.progress = None;
            app.last_progress_line = None;
            app.last_error = None;
            
            let (rx, tx) = core::run_with_events(cmd);
            app.stdin_tx = Some(tx);

            std::thread::spawn(move || {
                let mut had_error = false;
                for event in rx {
                    if matches!(event, FfmpegEvent::Error(_)) {
                        had_error = true;
                    }
                    let _ = event_tx.send(event);
                }
                let status = if had_error {
                    JobStatus::Failed
                } else {
                    JobStatus::Finished
                };
                let _ = job_tx.send(status);
            });
        }
        Ok(Commands::Probe(args)) => {
            let cmd = cli::probe_args_to_command(args);
            app.duration = parse_duration_from_args(&cmd.extra_args);
            app.job_running = true;
            app.job_status = Some(JobStatus::Running);
            app.progress = None;
            app.last_progress_line = None;
            app.last_error = None;

            let (rx, tx) = core::run_with_events(cmd);
            app.stdin_tx = Some(tx);

            std::thread::spawn(move || {
                let mut had_error = false;
                for event in rx {
                    if matches!(event, FfmpegEvent::Error(_)) {
                        had_error = true;
                    }
                    let _ = event_tx.send(event);
                }
                let status = if had_error {
                    JobStatus::Failed
                } else {
                    JobStatus::Finished
                };
                let _ = job_tx.send(status);
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
        Some(JobStatus::AwaitingConfirmation) => "Awaiting Confirmation",
        None => "Idle",
    };

    let progress = match &app.progress {
        Some(update) => format!(
            "time={} frame={} speed={}x",
            format_duration(update.time),
            update.frame,
            update.speed
        ),
        None => "time=--:--:-- frame= speed=".to_string(),
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

    if let (Some(update), Some(total)) = (&app.progress, app.duration) {
        let elapsed = update.time.as_secs_f64();
        let total = total.as_secs_f64();
        if total > 0.0 {
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

fn parse_duration_from_args(args: &[String]) -> Option<Duration> {
    let mut idx = 0;
    while idx < args.len() {
        if args[idx] == "-t" {
            if let Some(value) = args.get(idx + 1) {
                if let Ok(seconds) = value.parse::<f64>() {
                    let micros = (seconds * 1_000_000.0).round().max(0.0) as u64;
                    return Some(Duration::from_micros(micros));
                }
                if let Some(duration) = parse_ffmpeg_time(value) {
                    return Some(duration);
                }
            }
        }
        if let Some(pos) = args[idx].find("duration=") {
            let value = &args[idx][pos + "duration=".len()..];
            let value = value.split(':').next().unwrap_or(value);
            if let Ok(seconds) = value.parse::<f64>() {
                let micros = (seconds * 1_000_000.0).round().max(0.0) as u64;
                return Some(Duration::from_micros(micros));
            }
        }
        idx += 1;
    }
    None
}
