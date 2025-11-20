//! A common terminal UI implementation.
use framework::prelude::*;
use std::any::Any;
use ratatui::layout::{Layout, Constraint};
use crossterm::event::{Event, KeyEventKind, KeyEvent, KeyCode};
use ratatui::style::{Stylize};
use ratatui::text::{Line};
use ratatui::widgets::{Block, Paragraph, Borders, Tabs, Widget};
use ratatui::buffer::Buffer;

#[derive(Default)]
pub struct Tui {
    quit_requested: bool,
    tabs: Vec<Tab>,
    selected_tab: usize
}

impl Tui {
    pub fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    pub fn should_quit(&self) -> bool {
        self.quit_requested
    }

    pub fn with_tabs(mut self, tabs: impl Into<Vec<Tab>>) -> Self {
        self.tabs = tabs.into();
        self
    }

    fn cycle_tab_next(&mut self) {
        self.selected_tab = (self.selected_tab + 1) % self.tabs.len();
    }

    fn cycle_tab_prev(&mut self) {
        self.selected_tab = (self.selected_tab - 1) % self.tabs.len();
    }
}

impl Resource for Tui {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

pub struct Tab {
    title: String
}

impl Tab {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into()
        }
    }
}

// TODO: use a better result type
pub fn run_tui(context: &mut Context) -> std::result::Result<(), ()> {
    
    let mut terminal = ratatui::init();
    let result = loop {
        terminal.draw(|f| render_tui(context, f)).map_err(|_| ())?;
        let ev = crossterm::event::read().map_err(|_| ())?;
        handle_event(context, ev);
        if context.get_resource_mut::<Tui>().unwrap().should_quit() {
            break Ok(());
        }
    };
    ratatui::restore();
    result
}

fn render_tui(context: &mut Context, frame: &mut ratatui::Frame) {
    let vertical_layout = Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)]);
    let [header_area, content_area, footer_area] = vertical_layout.areas(frame.area());
    //frame.render_widget(Line::from("Training Assistant TUI").bold().centered(), header_area);
    render_tabs(context, frame);
    frame.render_widget(
        Paragraph::new("content")
            .block(Block::new().borders(Borders::ALL)),
        content_area
    );
    frame.render_widget(Line::from("<Q> Quit <Left> Prev Tab <Right> Next Tab").centered(), footer_area);
}

fn render_tabs(context: &mut Context, frame: &mut ratatui::Frame) {
    let titles = context.get_resource_mut::<Tui>().unwrap().tabs.iter()
        .map(|t| t.title.clone());
    Tabs::new(titles)
        .select(context.get_resource_mut::<Tui>().unwrap().selected_tab)
        .render(frame.area(), frame.buffer_mut());
}

fn handle_event(context: &mut Context, ev: crossterm::event::Event) {
    match ev {
        Event::Key(key_event) => {
            if key_event.kind == KeyEventKind::Press {
                if key_event.code == KeyCode::Char('q') {
                    context.get_resource_mut::<Tui>().unwrap().request_quit();
                }
                if key_event.code == KeyCode::Left {
                    context.get_resource_mut::<Tui>().unwrap().cycle_tab_prev();
                }
                if key_event.code == KeyCode::Right {
                    context.get_resource_mut::<Tui>().unwrap().cycle_tab_next();
                }
            }
        }
        _ => { }
    }
}
