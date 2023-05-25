use anyhow::{anyhow, Result};
use checkmate::{JobRunner, JobThread, Task, TaskResult};
use std::process::{ExitStatus, Output};
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
    Frame,
};

pub struct State {
    pub job_table: TableState,
    pub draw_mode: DrawMode,
}

impl Default for State {
    fn default() -> Self {
        let mut job_table = TableState::default();
        job_table.select(Some(0));

        Self {
            job_table,
            draw_mode: DrawMode::Job,
        }
    }
}

impl State {
    pub fn up_key(&mut self) {
        self.job_table.select(
            self.job_table
                .selected()
                .map(|x| (x.saturating_sub(1)).max(0)),
        );
    }

    pub fn down_key(&mut self, max: usize) {
        self.job_table
            .select(self.job_table.selected().map(|x| (x + 1).min(max)));
    }

    pub fn enter_key(&mut self) {
        self.draw_mode = DrawMode::Task;
    }

    pub fn back_key(&mut self) {
        self.draw_mode = DrawMode::Job;
    }

    pub fn draw<B: Backend>(&mut self, f: &mut Frame<B>, runner: &JobRunner) {
        match self.draw_mode {
            DrawMode::Job => self.draw_job(f, runner),
            DrawMode::Task => self.draw_task(f, runner),
        }
    }

    fn draw_job<B: Backend>(&mut self, f: &mut Frame<B>, runner: &JobRunner) {
        let columns: Vec<(&str, Constraint, fn(&JobThread) -> String)> = vec![
            ("Task", Constraint::Percentage(20), |jt| jt.task.name()),
            ("Status", Constraint::Percentage(6), |jt| {
                jt.runners.iter().fold(String::new(), |acc, r| {
                    match r.status() {
                        Some(s) => {
                            if s.success() {
                                "Complete"
                            } else {
                                "Failed"
                            }
                        }
                        _ => "In Progress",
                    }
                    .to_string()
                })
            }),
            ("Type", Constraint::Percentage(14), |jt| jt.task.type_name()),
            ("Output", Constraint::Percentage(60), |jt| {
                jt.runners.iter().fold(String::new(), |acc, r| {
                    format!(
                        "{}{:?}",
                        acc,
                        String::from_utf8(r.stdout()).expect("Failed to stringify output")
                    )
                })
            }),
        ];
        let rows: Vec<Row> = runner
            .threads
            .iter()
            .map(|jt| {
                columns
                    .iter()
                    .map(|(_, _, f)| f(jt))
                    .map(|s| Cell::from(s))
                    .collect()
            })
            .map(|x: Vec<Cell>| Row::new(x))
            .collect();

        let widths = columns
            .iter()
            .map(|(_, width, _)| *width)
            .collect::<Vec<Constraint>>();

        let table = Table::new(rows)
            .block(
                Block::default()
                    .title(format!("Job: {}", runner.job.name))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .widths(&widths)
            .highlight_style(Style::default().bg(Color::Rgb(40, 40, 90)))
            .highlight_symbol("> ")
            .column_spacing(1)
            .header(
                Row::new(
                    columns
                        .iter()
                        .map(|(title, _, _)| title)
                        .map(|x| Cell::from(*x)),
                )
                .bottom_margin(1)
                .style(Style::default().add_modifier(Modifier::BOLD)),
            );

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Percentage(95), Constraint::Min(1)].as_ref())
            .split(f.size());

        f.render_stateful_widget(table, chunks[0], &mut self.job_table);
        f.render_widget(Self::help(), chunks[1]);
    }

    fn draw_task<B: Backend>(&mut self, f: &mut Frame<B>, runner: &JobRunner) {
        let thread = &runner.threads[self.job_table.selected().expect("NO SELECTION")];

        let output = thread.runners.iter().fold(String::new(), |acc, r| {
            format!(
                "{}{}",
                acc,
                String::from_utf8(r.stdout()).expect("Failed to stringify output")
            )
        });
        let output = Text::from(output);
        let status = Span::from("STATUS");

        let paragraph = Paragraph::new(output)
            .block(
                Block::default()
                    .title(Spans::from(vec![
                        Span::raw(format!(
                            "Job: {} - Task[{}]: {} - ",
                            runner.job.name,
                            self.job_table.selected().expect(""),
                            runner.job.tasks[self.job_table.selected().expect("")].name()
                        )),
                        status,
                    ]))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Percentage(95), Constraint::Min(1)].as_ref())
            .split(f.size());

        f.render_widget(paragraph, chunks[0]);
        f.render_widget(Self::help(), chunks[1]);
    }

    fn help<'a>() -> Paragraph<'a> {
        let commands = vec![
            "<ctrl+c>: Quit",
            "<↑/↓>: Navigate",
            "<enter>: View full logs",
            "<esc> Go back to Job view",
        ];

        let text = vec![Spans::from(vec![Span::raw(commands.join("    "))])];

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title("")
                    .borders(Borders::NONE)
                    .border_type(BorderType::Rounded),
            )
            // .style(Style::default().fg(Color::White).bg(Color::Black))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        paragraph
    }

    fn title<'a>() -> Paragraph<'a> {
        let text = vec![Spans::from(vec![Span::raw("♚ Checkmate ♔")])];

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title("")
                    .borders(Borders::NONE)
                    .border_type(BorderType::Rounded),
            )
            // .style(Style::default().fg(Color::White).bg(Color::Black))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        paragraph
    }
}

pub enum DrawMode {
    Job,
    Task,
}
