use crossterm::event::KeyEventKind;
use crossterm::event::{Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{event, execute};
use ratatui::prelude::{Alignment, Color, Rect};
use ratatui::prelude::{Constraint, CrosstermBackend, Direction, Layout, Modifier};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Paragraph;
use std::env::args;
use std::error::Error;
use std::{
    io::{self, Write},
    thread,
    time::{Duration, Instant},
};

use ratatui::Terminal;
use ratatui::widgets::{Borders, List, ListItem};
use sysinfo::{Pid, PidExt, ProcessExt, System, SystemExt};
fn main() {
    let mut x: Vec<String> = args().collect();
    x.remove(0);
    println!("{:?}", grim_command(x).unwrap());
}
pub fn grim_command(args: Vec<String>) -> Result<String, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut force = false;
    let mut interactive = false;
    let mut kill_children = false;
    let mut exact = false;
    let mut watch = false;
    let mut interval = 2;
    let mut max_kills = None;
    let mut timeout = None;
    let mut targets: Vec<String> = vec![];

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" => return Ok(print_help()),
            "--force" => force = true,
            "--interactive" => interactive = true,
            "--kill-children" => kill_children = true,
            "--exact" => exact = true,
            "--watch" => watch = true,
            "--interval" => {
                i += 1;
                if i < args.len() {
                    interval = args[i].parse().unwrap_or(2);
                }
            }
            "--max" => {
                i += 1;
                if i < args.len() {
                    max_kills = Some(args[i].parse().unwrap_or(0));
                }
            }
            "--timeout" => {
                i += 1;
                if i < args.len() {
                    timeout = Some(args[i].parse().unwrap_or(0));
                }
            }
            other => targets.push(other.to_string()),
        }
        i += 1;
    }

    if interactive {
        let x = grim_interactive();
        match x {
            Ok(_) => {}
            Err(e) => {
                eprintln!("{}", e)
            }
        }
        return Ok("".to_string());
    }
    if targets.is_empty() {
        return Err("Missing targets for grim (PIDs or process names)".to_string());
    }

    let start_time = Instant::now();
    let mut total_killed = 0;

    loop {
        sys.refresh_all();
        let mut processes_to_kill: Vec<(Pid, String, String)> = vec![];

        for target in &targets {
            if let Ok(pid) = target.parse::<i32>() {
                if let Some(proc) = sys.process(Pid::from_u32(pid as u32)) {
                    processes_to_kill.push((
                        proc.pid(),
                        proc.name().to_string(),
                        proc.cmd().join(" "),
                    ));
                }
            } else {
                let pattern = target.to_lowercase();
                for (proc_pid, proc) in sys.processes() {
                    let name = proc.name().to_lowercase();
                    let cmdline = proc.cmd().join(" ").to_lowercase();
                    let matched = if exact {
                        name == pattern
                    } else {
                        name.contains(&pattern) || cmdline.contains(&pattern)
                    };
                    if matched {
                        processes_to_kill.push((
                            *proc_pid,
                            proc.name().to_string(),
                            proc.cmd().join(" "),
                        ));
                    }
                }
            }
        }

        for (pid, name, cmd) in &processes_to_kill {
            sys.refresh_process(*pid);

            if let Some(proc) = sys.process(*pid) {
                let cpu_usage = proc.cpu_usage();
                let memory_mb = proc.memory() as f64 / 1024.0;
                let uptime = proc.run_time();
                let parent_pid = proc.parent().map(|p| p.as_u32()).unwrap_or(0);

                println!("\n🔍 Found process:");
                println!("    PID:        {}", pid.as_u32());
                println!("    Name:       {}", name);
                println!("    Cmd:        {}", cmd);
                println!("    CPU usage:  {:.2}%", cpu_usage);
                println!("    Memory:     {:.2} MB", memory_mb);
                println!("    Uptime:     {} sec", uptime);
                println!("    Parent PID: {}", parent_pid);

                let mut children = vec![];
                for (child_pid, child_proc) in sys.processes() {
                    if let Some(ppid) = child_proc.parent() {
                        if ppid == *pid {
                            children.push((*child_pid, child_proc.name().to_string()));
                        }
                    }
                }
                if !children.is_empty() {
                    println!("⚠️  Has {} child(ren):", children.len());
                    for (cpid, cname) in &children {
                        println!("      ↳ PID {} - {}", cpid.as_u32(), cname);
                    }
                } else {
                    println!("    Child processes: (none)");
                }

                if !force {
                    print!(
                        "⚠️  Kill this process{}? (y/N): ",
                        if kill_children && !children.is_empty() {
                            " and its children"
                        } else {
                            ""
                        }
                    );
                    io::stdout().flush().unwrap();
                    let mut input = String::new();
                    io::stdin().read_line(&mut input).unwrap();
                    if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                        println!("⏭️  Skipping PID {}", pid.as_u32());
                        continue;
                    }
                }

                if kill_children {
                    for (cpid, cname) in &children {
                        if let Some(child_proc) = sys.process(*cpid) {
                            if child_proc.kill() {
                                println!("✅ Killed child PID {} - {}", cpid.as_u32(), cname);
                                total_killed += 1;
                            }
                        }
                    }
                }

                if proc.kill() {
                    println!("✅ Killed PID {} ({})", pid.as_u32(), name);
                    total_killed += 1;
                }
            }
        }

        if let Some(max) = max_kills {
            if total_killed >= max {
                println!("🎉 Reached max kill count ({}). Exiting.", max);
                break;
            }
        }

        if let Some(timeout_secs) = timeout {
            if start_time.elapsed().as_secs() >= timeout_secs as u64 {
                println!("⏰ Timeout of {}s reached. Exiting.", timeout_secs);
                break;
            }
        }

        if !watch {
            break;
        }

        for sec in (1..=interval).rev() {
            print!("\r⏳ Checking again in {}... ", sec);
            io::stdout().flush().unwrap();
            thread::sleep(Duration::from_secs(1));
        }
        println!();
    }

    println!("🎯 Finished. Total processes killed: {}", total_killed);
    Ok("".to_string())
}
fn print_help() -> String {
    // ANSI helpers
    const RESET: &str = "\x1b[0m";
    const BOLD: &str = "\x1b[1m";

    const TITLE: &str = "\x1b[38;5;213m"; // pink/purple
    const SECTION: &str = "\x1b[38;5;81m"; // cyan
    const OPTION: &str = "\x1b[38;5;214m"; // orange
    const ARG: &str = "\x1b[38;5;150m"; // green
    const DESC: &str = "\x1b[38;5;250m"; // light gray

    const BG_OPTION: &str = "\x1b[48;5;235m"; // dark background

    println!(
        r#"
{TITLE}{BOLD}grim{RESET}{DESC} — interactive and scripted process terminator{RESET}

{SECTION}{BOLD}USAGE:{RESET}
  {ARG}grim{RESET} {OPTION}[OPTIONS]{RESET} {ARG}<TARGET>...{RESET}

{SECTION}{BOLD}TARGETS:{RESET}
  {ARG}PID{RESET}                    {DESC}Kill a specific process by PID{RESET}
  {ARG}NAME{RESET}                   {DESC}Match process name or command line{RESET}

{SECTION}{BOLD}OPTIONS:{RESET}
  {BG_OPTION}{OPTION} --interactive {RESET}          {DESC}Launch full-screen interactive TUI{RESET}
  {BG_OPTION}{OPTION} --force {RESET}                {DESC}Kill without confirmation{RESET}
  {BG_OPTION}{OPTION} --kill-children {RESET}        {DESC}Also terminate child processes{RESET}
  {BG_OPTION}{OPTION} --exact {RESET}                {DESC}Match process name exactly{RESET}
  {BG_OPTION}{OPTION} --watch {RESET}                {DESC}Continuously monitor and kill matching processes{RESET}

  {BG_OPTION}{OPTION} --interval {ARG}<seconds>{RESET}    {DESC}Watch mode refresh interval (default: 2){RESET}
  {BG_OPTION}{OPTION} --max {ARG}<count>{RESET}           {DESC}Stop after killing N processes{RESET}
  {BG_OPTION}{OPTION} --timeout {ARG}<seconds>{RESET}     {DESC}Stop after a time limit{RESET}

  {BG_OPTION}{OPTION} --help {RESET}                 {DESC}Print this help message{RESET}
"#,
        TITLE = TITLE,
        SECTION = SECTION,
        OPTION = OPTION,
        ARG = ARG,
        DESC = DESC,
        RESET = RESET,
        BOLD = BOLD,
        BG_OPTION = BG_OPTION,
    );
    "".to_string()
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn grim_interactive() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut confirm_dialog: Option<(Pid, bool)> = None;
    let mut sys = System::new_all();
    let mut selected_idx = 0;
    let mut total_killed = 0;
    let mut kill_children = false;
    let mut force = false;

    let mut filter = String::new();
    let mut filter_mode = false;

    loop {
        sys.refresh_all();
        let all_processes: Vec<(Pid, String, f32, u64, u64)> = sys
            .processes()
            .iter()
            .map(|(pid, proc)| {
                (
                    *pid,
                    proc.name().to_string(),
                    proc.cpu_usage(),
                    proc.memory(),
                    proc.run_time(),
                )
            })
            .collect();

        let processes: Vec<_> = if !filter.is_empty() {
            all_processes
                .into_iter()
                .filter(|(_, name, _, _, _)| name.to_lowercase().contains(&filter.to_lowercase()))
                .collect()
        } else {
            all_processes
        };

        terminal.draw(|f| {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints(if filter_mode {
                    vec![Constraint::Length(3), Constraint::Min(1), Constraint::Length(2)]
                } else {
                    vec![Constraint::Min(1), Constraint::Length(2)]
                })
                .split(f.area());

            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                .split(layout[if filter_mode {1} else {0}]);

            let mut state = ratatui::widgets::ListState::default();
            if !processes.is_empty() {
                selected_idx = selected_idx.min(processes.len() - 1);
                state.select(Some(selected_idx));
            }

            let items: Vec<ListItem> = processes.iter()
                .map(|(pid, name, cpu, mem, uptime)| {
                    let cpu_color = if *cpu < 20.0 { Color::Green }
                    else if *cpu < 50.0 { Color::Yellow }
                    else { Color::Red };
                    let mem_color = if *mem < 50_000 { Color::Green }
                    else if *mem < 200_000 { Color::Yellow }
                    else { Color::Red };

                    ListItem::new(Line::from(vec![
                        Span::raw(format!("PID: {:<5} {:<15} CPU:", pid.as_u32(), name)),
                        Span::styled(format!("{:>4.1}%", cpu), Style::default().fg(cpu_color)),
                        Span::raw(" MEM:"),
                        Span::styled(format!("{:>6} KB", mem), Style::default().fg(mem_color)),
                        Span::raw(format!(" UP: {}s", uptime)),
                    ]))
                }).collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL)
                    .title(format!("Processes [{} shown]", processes.len())))
                .highlight_symbol("➤ ");
            f.render_stateful_widget(list, main_chunks[0], &mut state);

            let details = if let Some((pid, name, _, _, _)) = processes.get(selected_idx) {
                let proc = sys.process(*pid).unwrap();
                let parent_pid = proc.parent().map(|p| p.as_u32()).unwrap_or(0);
                let mut children = String::new();
                for (child_pid, child_proc) in sys.processes() {
                    if child_proc.parent() == Some(*pid) {
                        children.push_str(&format!(" ↳ {} ({})\n", child_pid.as_u32(), child_proc.name()));
                    }
                }
                if children.is_empty() { children.push_str(" (none)\n"); }

                format!(
                    "PID: {}\nName: {}\nCMD: {}\nParent PID: {}\nCPU: {:.2}%\nMEM: {} KB\nUptime: {}s\nChildren:\n{}",
                    pid.as_u32(), name, proc.cmd().join(" "), parent_pid,
                    proc.cpu_usage(), proc.memory(), proc.run_time(), children
                )
            } else { "No process selected.".to_string() };

            f.render_widget(Paragraph::new(details)
                                .block(Block::default().borders(Borders::ALL).title("Details")), main_chunks[1]);

            let footer_text = format!(
                "[↑↓] Move  [k] Kill  [c] children={} [f] force={} [w]  [/] filter  [q] Quit | Total killed: {}",
                kill_children, force, total_killed
            );
            f.render_widget(Paragraph::new(footer_text)
                                .block(Block::default().borders(Borders::TOP)), *layout.last().unwrap());

            if filter_mode {
                f.render_widget(Paragraph::new(filter.clone())
                                    .block(Block::default().borders(Borders::ALL).title("Filter (ESC to cancel)")),
                                layout[0]);
            }

            if let Some((pid, yes_selected)) = &confirm_dialog {
                let area = centered_rect(60, 20, f.area());
                let title = if let Some(x) = sys.process(*pid) {
                    &format!("Kill PID {} ({})?", pid.as_u32(), x.name())
                } else {
                    ""                    
                };
                let block = Block::default()
                    .title(title)
                    .borders(Borders::ALL);
                let line = Line::from(vec![
                    Span::styled("  [ Yes ]  ",
                                 if *yes_selected { Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default() }),
                    Span::raw("    "),
                    Span::styled("  [ No ]  ",
                                 if !*yes_selected { Style::default().fg(Color::Black).bg(Color::Red).add_modifier(Modifier::BOLD) } else { Style::default() }),
                ]);
                f.render_widget(Paragraph::new(line).block(block).alignment(Alignment::Center), area);
            }
        })?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if let Some((pid, mut yes_selected)) = confirm_dialog {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Left | KeyCode::Right => {
                                yes_selected = !yes_selected;
                                confirm_dialog = Some((pid, yes_selected));
                            }
                            KeyCode::Enter => {
                                if yes_selected {
                                    if kill_children {
                                        for child_proc in sys.processes().values() {
                                            if child_proc.parent() == Some(pid) && child_proc.kill()
                                            {
                                                total_killed += 1;
                                            }
                                        }
                                    }
                                    if let Some(proc) = sys.process(pid) {
                                        if proc.kill() {
                                            total_killed += 1;
                                        }
                                    }
                                }
                                confirm_dialog = None;
                            }
                            KeyCode::Esc => confirm_dialog = None,
                            _ => {
                                confirm_dialog = Some((pid, yes_selected));
                            }
                        }
                    }
                    continue;
                }

                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('/') => {
                            filter_mode = true;
                            filter.clear();
                        }
                        KeyCode::Enter if filter_mode => filter_mode = false,
                        KeyCode::Esc if filter_mode => {
                            filter_mode = false;
                            filter.clear();
                        }
                        KeyCode::Char(c) if filter_mode => filter.push(c),
                        KeyCode::Backspace if filter_mode => {
                            filter.pop();
                        }

                        KeyCode::Char('q') => break,
                        KeyCode::Down if !filter_mode => {
                            if selected_idx + 1 < processes.len() {
                                selected_idx += 1;
                            }
                        }
                        KeyCode::Up if !filter_mode => {
                            selected_idx = selected_idx.saturating_sub(1);
                        }
                        KeyCode::Char('c') if !filter_mode => kill_children = !kill_children,
                        KeyCode::Char('f') if !filter_mode => force = !force,

                        KeyCode::Char('k') if !filter_mode => {
                            if let Some((pid, _, _, _, _)) = processes.get(selected_idx) {
                                if force {
                                    if kill_children {
                                        for child_proc in sys.processes().values() {
                                            if child_proc.parent() == Some(*pid)
                                                && child_proc.kill()
                                            {
                                                total_killed += 1;
                                            }
                                        }
                                    }
                                    if let Some(proc) = sys.process(*pid) {
                                        if proc.kill() {
                                            total_killed += 1;
                                        }
                                    }
                                } else {
                                    confirm_dialog = Some((*pid, true));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    println!("🎯 Done. Total processes killed: {}", total_killed);
    Ok(())
}
