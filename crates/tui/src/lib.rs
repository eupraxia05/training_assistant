//! A common terminal UI implementation.
use framework::prelude::*;
use std::any::Any;
use ratatui::layout::{Rect, Layout, Constraint};
use crossterm::event::{Event, KeyEventKind, KeyEvent, KeyCode};
use ratatui::style::{Stylize, Style, Color};
use ratatui::text::{Line, Text, Span};
use ratatui::widgets::{Block, Paragraph, Borders, Tabs, Widget, List};
use ratatui::buffer::Buffer;
use ratatui::Frame;
use clap::{ArgMatches, Command};

#[derive(Clone)]
pub struct TuiPlugin;

impl Plugin for TuiPlugin {
    fn build(self, context: &mut Context) {
        context.add_resource(TuiNewTabTypes::default());
        context.add_command(
            Command::new("tui")
                .about("Opens an empty TUI session."),
            process_tui_command
        );
    }
}

#[derive(Default)]
pub struct Tui {
    quit_requested: bool,
    tabs: Vec<Tab>,
    selected_tab: usize,
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

struct TuiNewTabType {
    name: String,
    funcs: TabFuncs
}

#[derive(Default)]
pub struct TuiNewTabTypes {
    types: Vec<TuiNewTabType>
}

impl TuiNewTabTypes {
    pub fn register_new_tab_type<T>(&mut self, name: impl Into<String>)
        where T: TabImpl
    {
        self.types.push(TuiNewTabType {
            name: name.into(),
            funcs: TabFuncs::new::<T>()
        });
    }
}

impl Resource for TuiNewTabTypes {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

fn process_db_info_command(
    db_connection: &mut DbConnection
) -> Result<CommandResponse> {
    let mut response_text = String::default();
    if db_connection.is_open() {
        response_text += "Database connection open.\n";
        if let Some(db_path) = db_connection.db_path() {
            response_text += format!("Database path: {:?}", db_path).as_str();
        } else {
            response_text += "No database path (in-memory connection)";
        }
    } else {
        response_text += "No database connection open.";
    }

    Ok(CommandResponse::new(response_text))
}
pub struct Tab {
    title: String,
    funcs: Option<TabFuncs>,
}

impl Tab {
    pub fn new<T>(title: impl Into<String>) -> Self 
        where T : TabImpl
    {
        Self {
            title: title.into(),
            funcs: Some(TabFuncs::new::<T>())
        }
    }

    pub fn new_empty(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            funcs: None
        }
    }
}

pub trait TabImpl {
    fn title() -> String;
    fn render(context: &mut Context, buffer: &mut Buffer, area: Rect, block: Block);
}

type TabTitleFn = fn () -> String;
type TabRenderFn = fn (&mut Context, &mut Buffer, Rect, Block);

struct TabFuncs {
   title_fn: TabTitleFn,
   render_fn: TabRenderFn
}

impl TabFuncs {
    fn new<T>() -> Self 
        where T : TabImpl
    {
        Self {
            title_fn: T::title,
            render_fn: T::render,
        }
    }
}

pub struct KeyBind {
    display_key: String,
    display_name: String,
}

impl KeyBind {
    fn display_text(&self) -> Line {
        Line::from(vec![
           Span::styled(format!("<{}>", self.display_key), Style::default().fg(Color::Black).bg(Color::White)),
           Span::styled(format!(" {}", self.display_name), Style::default())
        ])
    }
}

// TODO: use a better result type
pub fn run_tui(context: &mut Context) -> std::result::Result<(), ()> { 
    let mut terminal = ratatui::init();
    let result = loop {
        
        let global_keybinds = vec!(
            KeyBind {
                display_key: "Q".into(),
                display_name: "Quit".into()
            },
            KeyBind {
                display_key: "Ctrl+Left".into(),
                display_name: "Prev Tab".into(),
            },
            KeyBind {
                display_key: "Ctrl+Right".into(),
                display_name: "Next Tab".into(),
            },
            KeyBind {
                display_key: "Ctrl+T".into(),
                display_name: "New Tab".into(),
            }
        );

        terminal.draw(|f| render_tui(context, f, &global_keybinds)).map_err(|_| ())?;
        let ev = crossterm::event::read().map_err(|_| ())?;
        handle_event(context, ev);
        if context.get_resource_mut::<Tui>().unwrap().should_quit() {
            break Ok(());
        }
    };
    ratatui::restore();
    result
}

fn render_tui(context: &mut Context, frame: &mut ratatui::Frame, keybinds: &Vec<KeyBind>) {
    let mut keybind_lines = vec!(Vec::new());
    let mut width_so_far = 0;
    for keybind in keybinds {
       let text = keybind.display_text();
       if width_so_far + text.width() + 1 < frame.area().width as usize {
           width_so_far += text.width() + 1;
           keybind_lines.last_mut().unwrap().push(text);
       } else {
           width_so_far = text.width() + 1;
           keybind_lines.push(vec!(text));
       }
    };

    let vertical_layout = Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(keybind_lines.len() as u16)]);
    let [header_area, content_area, footer_area] = vertical_layout.areas(frame.area());
    //frame.render_widget(Line::from("Training Assistant TUI").bold().centered(), header_area);
    render_tabs(context, frame);
    let selected_tab = context.get_resource_mut::<Tui>().unwrap().selected_tab;
    let tab_funcs = if let Some(funcs) = &context.get_resource_mut::<Tui>().unwrap().tabs[selected_tab].funcs {
        funcs
    } else {
        &TabFuncs::new::<EmptyTabImpl>()
    };
    (tab_funcs.render_fn)(context, frame.buffer_mut(), content_area, Block::new().borders(Borders::ALL));

    let keybind_vertical_layout_constraints = (0..keybind_lines.len()).map(|_| Constraint::Length(1));
    let keybind_vertical_layout = Layout::vertical(keybind_vertical_layout_constraints).split(footer_area);
    for (line_idx, keybind_line) in keybind_lines.iter().enumerate() {
        let col_constraints = (0..keybind_line.len()).map(|k| Constraint::Length(keybind_line[k].width().try_into().unwrap()));
        let horizontal_layout = Layout::horizontal(col_constraints).spacing(1).split(keybind_vertical_layout[line_idx]);
        for i in (0..keybind_line.len()) {
            frame.render_widget(Line::from(keybind_line[i].clone()), horizontal_layout[i]);
        }
    }
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

fn process_tui_command(context: &mut Context, arg_matches: &ArgMatches) -> Result<CommandResponse> {
    context.add_resource(Tui::default().with_tabs([Tab::new_empty("Empty Tab")]));
    Ok(CommandResponse::new("Opening TUI session..."))
}

struct EmptyTabImpl;

impl TabImpl for EmptyTabImpl {
    fn title() -> String {
        "Empty Tab".into()
    }

    fn render(context: &mut Context, buffer: &mut Buffer, rect: Rect, block: Block) {
        let tui_new_tab_types = context.get_resource_mut::<TuiNewTabTypes>().unwrap();
        let items = tui_new_tab_types.types.iter().map(|t| t.name.clone());
        if items.len() > 0 {
            List::new(items).block(block).render(rect, buffer);
        } else {
            Paragraph::new("No creatable tab types.").block(block).render(rect, buffer);
        }
    }
}
