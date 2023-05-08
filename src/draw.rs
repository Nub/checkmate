use anyhow::{anyhow, Result};
use checkmate::{JobRunner, Task, TaskResult};
use std::process::Output;
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
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
        let rows: Vec<Row> = runner
            .threads
            .iter()
            .map(|jr| {
                let (status, output) = match &(*jr.thread.borrow()) {
                    Ok(TaskResult::Script(Err(e))) => (
                        Cell::from("Failed").style(Style::default().fg(Color::Red)),
                        format!("{e:?}"),
                    ),
                    Ok(TaskResult::Script(Ok(x))) => (
                        Cell::from("Complete").style(Style::default().fg(Color::Green)),
                        String::from_utf8(x.stdout.clone()).expect("Failed to make string"),
                    ),
                    Ok(TaskResult::Serial(x)) => {
                        let errors = x.iter().fold(String::new(), |acc, x| {
                            if let Err(e) = x {
                                format!("{}:{}", acc, e)
                            } else {
                                acc
                            }
                        });

                        let status = if errors.len() != 0 {
                            Cell::from("Error").style(Style::default().fg(Color::Red))
                        } else {
                            Cell::from("Complete").style(Style::default().fg(Color::Green))
                        };
                        (
                            status,
                            x.iter()
                                .map(|x| match &x {
                                    Ok(x) => String::from_utf8(x.stdout.clone())
                                        .expect("Failed to make string"),
                                    Err(e) => format!("{e}"),
                                })
                                .collect::<Vec<String>>()
                                .join(" "),
                        )
                    }
                    Err(e) => (
                        Cell::from("In progress").style(Style::default().fg(Color::Blue)),
                        format!("{e}"),
                    ),
                    x => (
                        Cell::from("Unknown").style(Style::default()),
                        format!("{:?}", x),
                    ),
                };

                Row::new(vec![Cell::from(jr.task.name()), status, Cell::from(output)])
            })
            .collect();

        let table = Table::new(rows)
            .block(
                Block::default()
                    .title(format!("Job: {}", runner.job.name))
                    .borders(Borders::ALL),
            )
            // .style(Style::default().fg(Color::White))
            .widths(&[
                Constraint::Percentage(30),
                Constraint::Percentage(10),
                Constraint::Percentage(60),
            ])
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(40, 40, 90))
                    // .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ")
            .column_spacing(1)
            .header(Row::new(vec!["Task", "Status", "Output"]).bottom_margin(1));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Percentage(95), Constraint::Min(1)].as_ref())
            .split(f.size());

        f.render_stateful_widget(table, chunks[0], &mut self.job_table);
        f.render_widget(Self::help(), chunks[1]);
    }

    fn draw_task<B: Backend>(&mut self, f: &mut Frame<B>, runner: &JobRunner) {
        let thread = runner.threads[self.job_table.selected().expect("NO SELECTION")]
            .thread
            .borrow();
        let (status, output) = match &(*thread) {
            Ok(TaskResult::Script(Err(e))) => (
                Span::styled("Failed", Style::default().fg(Color::Red)),
                vec![Spans::from(vec![Span::raw(format!("{e:?}"))])],
            ),
            Ok(TaskResult::Script(Ok(x))) => (
                Span::styled("Complete", Style::default().fg(Color::Green)),
                vec![Spans::from(vec![Span::raw(
                    String::from_utf8(x.stdout.clone()).expect("Failed to make string"),
                )])],
            ),
            Ok(TaskResult::Serial(x)) => {
                let errors = x.iter().fold(String::new(), |acc, x| {
                    if let Err(e) = x {
                        format!("{}:{}", acc, e)
                    } else {
                        acc
                    }
                });

                let status = if errors.len() != 0 {
                    Span::styled("Error", Style::default().fg(Color::Red))
                } else {
                    Span::styled("Complete", Style::default().fg(Color::Green))
                };

                (
                    status.clone(),
                    x.iter()
                        .enumerate()
                        .map(|(i, x)| {
                            let task_name = if let Task::Serial(t) =
                                &runner.job.tasks[self.job_table.selected().expect("NO SELECTION")]
                            {
                                t[i].name.clone()
                            } else {
                                "".to_string()
                            };

                            let status = if x.is_err() {
                                Span::styled("Error", Style::default().fg(Color::Red))
                            } else {
                                Span::styled("Complete", Style::default().fg(Color::Green))
                            };

                            let output = match &x {
                                Ok(x) => String::from_utf8(x.stdout.clone())
                                    .expect("Failed to make string"),
                                Err(e) => format!("{e}"),
                            };

                            let title_text = format!("Task[{}] {} - ", i, task_name);
                            let title = Spans::from(vec![Span::raw(title_text.clone()), status]);

                            let mut lines: Vec<Spans> = output
                                .lines()
                                .map(|l| Spans::from(vec![Span::raw(String::from(l))]))
                                .collect();
                            lines.insert(0, title);
                            lines.push(Spans::from(vec![Span::raw("⎯".repeat(35))]));

                            lines
                        })
                        .flatten()
                        .collect(),
                )
            }
            Err(e) => (
                Span::styled("In progress", Style::default().fg(Color::Blue)),
                vec![Spans::from(vec![Span::raw(format!("{e}"))])],
            ),
        };

        let paragraph = Paragraph::new(output)
            .block(
                Block::default()
                    .title(Spans::from(vec![
                        Span::raw(format!(
                            "Job: {} - Task: {} - ",
                            runner.job.name,
                            runner.job.tasks[self.job_table.selected().expect("")].name()
                        )),
                        status,
                    ]))
                    .borders(Borders::ALL),
            )
            // .style(Style::default().fg(Color::White).bg(Color::Black))
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

        let text = vec![Spans::from(vec![Span::raw(commands.join(" ⎯⎯⎯  "))])];

        let paragraph = Paragraph::new(text)
            .block(Block::default().title("Commands").borders(Borders::ALL))
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
