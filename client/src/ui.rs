use std::io::Write;
use std::time::Instant;

use chrono::Local;
use crossterm::style::{Attribute, Color, SetAttribute, SetForegroundColor, ResetColor};

/// A single logged request.
#[derive(Clone, Debug)]
pub struct RequestEntry {
    pub timestamp: String,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub latency_ms: u64,
}

// ── Colours & symbols ──────────────────────────────────────────────

const CYAN: Color = Color::Rgb { r: 0, g: 210, b: 210 };
const GREEN: Color = Color::Rgb { r: 80, g: 220, b: 100 };
const YELLOW: Color = Color::Rgb { r: 255, g: 200, b: 60 };
const RED: Color = Color::Rgb { r: 255, g: 80, b: 80 };
const DIM: Color = Color::Rgb { r: 120, g: 120, b: 120 };
const WHITE: Color = Color::Rgb { r: 230, g: 230, b: 230 };
const MAGENTA: Color = Color::Rgb { r: 200, g: 120, b: 255 };

// ── Banner ─────────────────────────────────────────────────────────

pub fn print_banner() {
    let mut out = std::io::stdout();
    let _ = writeln!(out);
    let _ = write!(out, "{}", SetForegroundColor(CYAN));
    let _ = writeln!(out, "  ╭──────────────────────────────────────────╮");
    let _ = writeln!(out, "  │           🚀 TunnelX  v0.1.0            │");
    let _ = writeln!(out, "  ╰──────────────────────────────────────────╯");
    let _ = write!(out, "{}", ResetColor);
    let _ = writeln!(out);
}

// ── Step indicators ────────────────────────────────────────────────

pub fn print_step_start(step: u8, total: u8, message: &str) {
    let mut out = std::io::stdout();
    let _ = write!(out, "{}", SetForegroundColor(DIM));
    let _ = write!(out, "  [{step}/{total}] ");
    let _ = write!(out, "{}", SetForegroundColor(WHITE));
    let _ = writeln!(out, "{message}");
    let _ = write!(out, "{}", ResetColor);
}

pub fn print_step_done(message: &str) {
    let mut out = std::io::stdout();
    let _ = write!(out, "  {}", SetForegroundColor(GREEN));
    let _ = write!(out, "  ✔ ");
    let _ = write!(out, "{}", SetForegroundColor(WHITE));
    let _ = writeln!(out, "{message}");
    let _ = write!(out, "{}", ResetColor);
}

pub fn print_step_fail(message: &str) {
    let mut out = std::io::stdout();
    let _ = write!(out, "  {}", SetForegroundColor(RED));
    let _ = write!(out, "  ✘ ");
    let _ = writeln!(out, "{message}");
    let _ = write!(out, "{}", ResetColor);
}

// ── Interactive prompts ────────────────────────────────────────────

pub fn prompt_subdomain() -> Option<String> {
    let _ = write!(
        std::io::stdout(),
        "{}  ",
        SetForegroundColor(WHITE)
    );
    let _ = write!(std::io::stdout(), "{}", ResetColor);

    let input: String = dialoguer::Input::new()
        .with_prompt("  Enter subdomain (or press Enter for random)")
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default();

    let trimmed = input.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn prompt_duration() -> u64 {
    let options = &[
        " 5 minutes",
        "20 minutes",
        "30 minutes",
        " 1 hour",
    ];
    let durations = [300, 1200, 1800, 3600];

    let selection = dialoguer::Select::new()
        .with_prompt("  Select tunnel duration")
        .items(options)
        .default(2) // 30 min default
        .interact()
        .unwrap_or(2);

    durations[selection]
}

// ── Tunnel live display ────────────────────────────────────────────

pub fn print_tunnel_live(public_url: &str, port: u16, duration_secs: u64) {
    let mut out = std::io::stdout();
    let _ = writeln!(out);
    let _ = write!(out, "  {}", SetForegroundColor(GREEN));
    let _ = write!(out, "{}", SetAttribute(Attribute::Bold));
    let _ = writeln!(out, "  🌐 Tunnel is live!");
    let _ = write!(out, "{}", SetAttribute(Attribute::Reset));
    let _ = writeln!(out);
    let _ = write!(out, "  {}", SetForegroundColor(CYAN));
    let _ = write!(out, "  {}", SetAttribute(Attribute::Bold));
    let _ = write!(out, "  {public_url}");
    let _ = write!(out, "{}", SetAttribute(Attribute::Reset));
    let _ = write!(out, "{}", SetForegroundColor(DIM));
    let _ = writeln!(out, "  →  localhost:{port}");
    let _ = writeln!(out);

    // QR code
    print_qr_code(public_url);

    let _ = writeln!(out);
    let _ = write!(out, "{}", SetForegroundColor(WHITE));
    let _ = writeln!(
        out,
        "  📋 Share this URL with your friends!"
    );
    let dur = format_duration(duration_secs);
    let _ = write!(out, "{}", SetForegroundColor(YELLOW));
    let _ = writeln!(out, "  ⏱  Expires in: {dur}");
    let _ = write!(out, "{}", ResetColor);
    let _ = writeln!(out);
    let _ = write!(out, "{}", SetForegroundColor(DIM));
    let _ = writeln!(
        out,
        "  ─── Request Log ──────────────────────────────────────"
    );
    let _ = write!(out, "{}", ResetColor);
}

pub fn print_qr_code(url: &str) {
    use qrcode::QrCode;

    let code = match QrCode::new(url) {
        Ok(c) => c,
        Err(_) => return,
    };

    let string = code
        .render::<char>()
        .quiet_zone(false)
        .module_dimensions(2, 1)
        .build();

    let mut out = std::io::stdout();
    let _ = write!(out, "{}", SetForegroundColor(WHITE));
    for line in string.lines() {
        let _ = writeln!(out, "    {line}");
    }
    let _ = write!(out, "{}", ResetColor);
}

// ── Request log ────────────────────────────────────────────────────

pub fn print_request_entry(entry: &RequestEntry) {
    let mut out = std::io::stdout();
    let _ = write!(out, "{}", SetForegroundColor(DIM));
    let _ = write!(out, "  {} ", entry.timestamp);

    let method_color = match entry.method.as_str() {
        "GET" => GREEN,
        "POST" => YELLOW,
        "PUT" | "PATCH" => MAGENTA,
        "DELETE" => RED,
        _ => WHITE,
    };
    let _ = write!(out, "{}", SetForegroundColor(method_color));
    let _ = write!(out, "{:<7}", entry.method);

    let _ = write!(out, "{}", SetForegroundColor(WHITE));
    let _ = write!(out, "{:<30}", truncate_path(&entry.path, 30));

    let status_color = if entry.status < 300 {
        GREEN
    } else if entry.status < 400 {
        YELLOW
    } else {
        RED
    };
    let _ = write!(out, "{}", SetForegroundColor(status_color));
    let _ = write!(out, "{:<6}", entry.status);

    let _ = write!(out, "{}", SetForegroundColor(DIM));
    let _ = writeln!(out, "{}ms", entry.latency_ms);

    let _ = write!(out, "{}", ResetColor);
    let _ = out.flush();
}

pub fn now_timestamp() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

// ── Countdown timer ────────────────────────────────────────────────

pub fn print_countdown(remaining_secs: u64) {
    let mut out = std::io::stdout();
    let _ = write!(out, "\r{}", SetForegroundColor(DIM));
    let _ = write!(
        out,
        "  ─── [c] Copy URL  [q] Quit  ⏱ {} ───",
        format_duration(remaining_secs)
    );
    let _ = write!(out, "{}", ResetColor);
    let _ = out.flush();
}

pub fn print_expired() {
    let mut out = std::io::stdout();
    let _ = writeln!(out);
    let _ = writeln!(out);
    let _ = write!(out, "{}", SetForegroundColor(YELLOW));
    let _ = writeln!(out, "  ⏱  Tunnel expired. Session ended.");
    let _ = write!(out, "{}", ResetColor);
    let _ = writeln!(out);
}

pub fn print_disconnect() {
    let mut out = std::io::stdout();
    let _ = writeln!(out);
    let _ = write!(out, "{}", SetForegroundColor(DIM));
    let _ = writeln!(out, "  👋 Tunnel closed. Goodbye!");
    let _ = write!(out, "{}", ResetColor);
    let _ = writeln!(out);
}

pub fn print_clipboard_copied() {
    let mut out = std::io::stdout();
    let _ = write!(out, "\r");
    let _ = write!(out, "{}", SetForegroundColor(GREEN));
    let _ = write!(out, "  ✔ URL copied to clipboard!                              ");
    let _ = write!(out, "{}", ResetColor);
    let _ = writeln!(out);
    let _ = out.flush();
}

pub fn print_clipboard_failed() {
    let mut out = std::io::stdout();
    let _ = write!(out, "\r");
    let _ = write!(out, "{}", SetForegroundColor(RED));
    let _ = write!(out, "  ✘ Could not copy to clipboard (no clipboard available)  ");
    let _ = write!(out, "{}", ResetColor);
    let _ = writeln!(out);
    let _ = out.flush();
}

// ── Helpers ────────────────────────────────────────────────────────

pub fn copy_to_clipboard(text: &str) -> bool {
    match arboard::Clipboard::new() {
        Ok(mut clipboard) => clipboard.set_text(text).is_ok(),
        Err(_) => false,
    }
}

fn format_duration(secs: u64) -> String {
    let m = secs / 60;
    let s = secs % 60;
    if m >= 60 {
        let h = m / 60;
        let rm = m % 60;
        format!("{h}h {rm:02}m {s:02}s")
    } else {
        format!("{m}m {s:02}s")
    }
}

fn truncate_path(path: &str, max: usize) -> String {
    if path.len() <= max {
        path.to_string()
    } else {
        format!("{}…", &path[..max - 1])
    }
}

/// Compute remaining seconds from a start time and total duration.
pub fn remaining_secs(start: Instant, total_duration_secs: u64) -> u64 {
    let elapsed = start.elapsed().as_secs();
    total_duration_secs.saturating_sub(elapsed)
}
