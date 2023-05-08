use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap,
    },
    Frame,
};

use checkmate::{JobRunner, Task, TaskResult};

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
                        "".to_string(),
                    ),
                    Ok(TaskResult::Script(Ok(x))) => (
                        Cell::from("Complete").style(Style::default().fg(Color::Green)),
                        String::from_utf8(x.stdout.clone()).expect("Failed to make string"),
                    ),
                    Ok(TaskResult::Serial(x)) => (
                        Cell::from("Complete").style(Style::default().fg(Color::Green)),
                        x.iter()
                            .map(|x| {
                                String::from_utf8(x.as_ref().expect("xXXXx").stdout.clone())
                                    .expect("Failed to make string")
                            })
                            .collect::<Vec<String>>()
                            .join(" "),
                    ),
                    Err(e) => (
                        Cell::from("In progress").style(Style::default()),
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
                Constraint::Percentage(15),
                Constraint::Percentage(10),
                Constraint::Percentage(75),
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
                Cell::from("Failed").style(Style::default().fg(Color::Red)),
                "".to_string(),
            ),
            Ok(TaskResult::Script(Ok(x))) => (
                Cell::from("Complete").style(Style::default().fg(Color::Green)),
                String::from_utf8(x.stdout.clone()).expect("Failed to make string"),
            ),
            Ok(TaskResult::Serial(x)) => (
                Cell::from("Complete").style(Style::default().fg(Color::Green)),
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
                        format!(
                            "Task[{}] {}:\n{}",
                            i,
                            task_name,
                            String::from_utf8(x.as_ref().expect("xXXXx").stdout.clone())
                                .expect("Failed to make string")
                        )
                    })
                    .collect::<Vec<String>>()
                    .join("\n\n"),
            ),
            Err(e) => (
                Cell::from("In progress").style(Style::default()),
                format!("{e}"),
            ),
        };

        let text: Vec<Spans> = output
            .lines()
            .map(|l| Spans::from(vec![Span::raw(l)]))
            .collect();

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title(format!(
                        "Job: {} | \tTask: {}",
                        runner.job.name,
                        runner.job.tasks[self.job_table.selected().expect("")].name()
                    ))
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
        let text = vec![Spans::from(vec![Span::raw(
            "Q: Quit ⎯⎯⎯ Enter: View full log ⎯⎯⎯ Esc: Go back to Job view",
        )])];
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
