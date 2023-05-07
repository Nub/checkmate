use anyhow::{anyhow, Result};
use checkmate::{Destination, Job, Script, Task};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::Write;
use std::time::Instant;
use std::{io, thread, time::Duration};
use tui::{backend::CrosstermBackend, Terminal};

mod draw;
use draw::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    job: Option<String>,

    #[arg(long, default_value_t = false)]
    generate_json_schema: bool,

    #[arg(long, default_value_t = false)]
    generate_test_data: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.generate_json_schema {
        let schema = schemars::schema_for!(Job);
        println!("{}", serde_json::to_string_pretty(&schema)?);
        return Ok(());
    }

    if args.generate_test_data {
        return generate_test_data();
    }

    let job: Job = serde_json::from_reader(
        std::fs::File::open(args.job.unwrap()).expect("Failed to open job file"),
    )
    .expect("Failed to parse job");

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    let runner = job.run();
    let mut state = State::default();

    loop {
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Up => {
                        state.up_key();
                    }
                    KeyCode::Down => {
                        state.down_key(runner.job.tasks.len() - 1);
                    }
                    KeyCode::Enter => {
                        state.enter_key();
                    }
                    KeyCode::Esc | KeyCode::Backspace => {
                        state.back_key();
                    }
                    _ => (),
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        terminal.draw(|f| state.draw(f, &runner))?;
        thread::sleep(Duration::from_millis(100));
    }

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn generate_test_data() -> Result<()> {
    let test = Job {
        name: "Test".into(),
        tasks: vec![
            Task::Script(Script {
                name: "local: bash_version".into(),
                script: "bash --version".into(),
                ..Default::default()
            }),
            Task::Script(Script {
                name: "znix: bash_version".into(),
                script: "bash --version".into(),
                destination: Destination::Remote("zthayer@10.17.68.57".into()),
                ..Default::default()
            }),
            Task::Serial(vec![
                Script {
                    name: "write".into(),
                    script: "date >> /tmp/date.tmp".into(),
                    destination: Destination::Remote("zthayer@10.17.68.57".into()),
                    ..Default::default()
                },
                Script {
                    name: "read".into(),
                    script: "cat /tmp/date.tmp".into(),
                    destination: Destination::Remote("zthayer@10.17.68.57".into()),
                    ..Default::default()
                },
            ]),
        ],
    };

    let mut file = std::fs::File::create("test.json")?;
    file.write_all(serde_json::to_string_pretty(&test)?.as_bytes())?;

    Ok(())
}
