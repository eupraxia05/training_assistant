//! A common terminal UI implementation.
use framework::prelude::*;
use std::any::Any;
use ratatui::layout::{Rect, Layout, Constraint};
use crossterm::event::{Event, KeyModifiers, KeyEventKind, KeyEvent, KeyCode};
use ratatui::style::{Stylize, Style, Color, palette::tailwind};
use ratatui::text::{Line, Text, Span};
use ratatui::widgets::{Block, BorderType, Paragraph, StatefulWidget, HighlightSpacing, Borders, Tabs, Widget, List, ListState};
use ratatui::buffer::Buffer;
use ratatui::Frame;
use clap::{ArgMatches, Command};
use std::collections::HashMap;

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
    tabs: Vec<(usize, Tab)>,
    selected_tab: usize,
    next_tab_id: usize
}

impl Tui {
    pub fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    pub fn should_quit(&self) -> bool {
        self.quit_requested
    }

    fn cycle_tab_next(&mut self) {
        self.selected_tab = (self.selected_tab + 1) % self.tabs.len();
    }

    fn cycle_tab_prev(&mut self) {
        self.selected_tab = (self.selected_tab - 1) % self.tabs.len();
    }

    pub fn add_tab(tab: Tab, context: &mut Context) {
        let tab_funcs = match &tab.funcs {
            Some(f) => f.clone(),
            None => TabFuncs::new::<EmptyTabImpl>()
        };
        let next_tab_id = context.get_resource::<Tui>().unwrap().next_tab_id;
        context.get_resource_mut::<Tui>().unwrap().tabs.push((next_tab_id, tab));
        (tab_funcs.create_state_fn)(context, next_tab_id);
        context.get_resource_mut::<Tui>().unwrap().next_tab_id += 1;
    }

    pub fn set_tab(tab_id: usize, tab: Tab, context: &mut Context) {
        let tab_funcs = match &tab.funcs {
            Some(f) => f.clone(),
            None => TabFuncs::new::<EmptyTabImpl>()
        };
        context.get_resource_mut::<Tui>().unwrap().tabs.iter_mut().find(|t| t.0 == tab_id).unwrap().1 = tab;
        (tab_funcs.create_state_fn)(context, tab_id);
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

#[derive(Default)]
pub struct TabState<T>
    where T: Default
{
    states: HashMap<usize, T>
}

impl<T> TabState<T> 
    where T: Default
{
    fn add_state(&mut self, tab_id: usize) {
        self.states.insert(tab_id, T::default());
    }

    pub fn get_state_mut(&mut self, tab_id: usize) -> Option<&mut T> {
        self.states.get_mut(&tab_id)
    }

    pub fn get_state(&mut self, tab_id: usize) -> Option<&T> {
        self.states.get(&tab_id)
    }
}

impl<T> Resource for TabState<T>
    where T: 'static + Default
{
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
    type State: Default + 'static;
    fn title() -> String;
    fn render(context: &mut Context, buffer: &mut Buffer, area: Rect, block: Block, tab_id: usize);
    fn keybinds() -> Vec<KeyBind>;
    fn handle_key(context: &mut Context, bind_name: &str, tab_idx: usize);
}

type TabTitleFn = fn () -> String;
type TabRenderFn = fn (&mut Context, &mut Buffer, Rect, Block, usize);
type TabKeybindsFn = fn () -> Vec<KeyBind>;
type TabHandleKeyFn = fn(&mut Context, &str, usize);
type TabCreateStateFn = fn(&mut Context, tab_idx: usize);

fn create_state<S>(context: &mut Context, tab_idx: usize) 
    where S: 'static + Default
{
    if !context.has_resource::<TabState<S>>() {
        context.add_resource(TabState::<S>::default());
    }
    context.get_resource_mut::<TabState<S>>().unwrap().add_state(tab_idx);
}


#[derive(Clone)]
struct TabFuncs {
   title_fn: TabTitleFn,
   render_fn: TabRenderFn,
   keybinds_fn: TabKeybindsFn,
   handle_key_fn: TabHandleKeyFn,
   create_state_fn: TabCreateStateFn
}

impl TabFuncs {
    fn new<T>() -> Self 
        where T : TabImpl
    {
        Self {
            title_fn: T::title,
            render_fn: T::render,
            keybinds_fn: T::keybinds,
            handle_key_fn: T::handle_key,
            create_state_fn: create_state::<T::State>
        }
    }
}

#[derive(Clone)]
pub struct KeyBind {
    name: String,
    display_key: String,
    display_name: String,
    key_code: KeyCode,
    modifiers: KeyModifiers,
}

impl KeyBind {
    fn display_text(&self) -> Line<'_> {
        Line::from(vec![
           Span::styled(format!("<{}>", self.display_key), Style::default().fg(Color::Black).bg(Color::White)),
           Span::styled(format!(" {}", self.display_name), Style::default())
        ])
    }
}

pub struct TuiStyle {
    
}

impl Resource for TuiStyle {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// TODO: use a better result type
pub fn run_tui(context: &mut Context) -> std::result::Result<(), ()> { 
    let mut terminal = ratatui::init();
    let result = loop {
        let global_keybinds = vec!(
            KeyBind {
                name: "quit".into(),
                display_key: "Q".into(),
                display_name: "Quit".into(),
                key_code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE
            },
            KeyBind {
                name: "prev_tab".into(),
                display_key: "Ctrl+Left".into(),
                display_name: "Prev Tab".into(),
                key_code: KeyCode::Left,
                modifiers: KeyModifiers::CONTROL,
            },
            KeyBind {
                name: "next_tab".into(),
                display_key: "Ctrl+Right".into(),
                display_name: "Next Tab".into(),
                key_code: KeyCode::Right,
                modifiers: KeyModifiers::CONTROL,
            },
            KeyBind {
                name: "new_tab".into(),
                display_key: "Ctrl+T".into(),
                display_name: "New Tab".into(),
                key_code: KeyCode::Char('t'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyBind {
                name: "close_tab".into(),
                display_key: "Ctrl+E".into(),
                display_name: "Close Tab".into(),
                key_code: KeyCode::Char('e'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyBind {
                name: "clear_tab".into(),
                display_key: "Ctrl+R".into(),
                display_name: "Clear Tab".into(),
                key_code: KeyCode::Char('r'),
                modifiers: KeyModifiers::CONTROL,
            }
        );
 
        let selected_tab = context.get_resource_mut::<Tui>().unwrap().selected_tab;
        let tab_funcs = if let Some(funcs) = &context.get_resource_mut::<Tui>().unwrap().tabs[selected_tab].1.funcs {
            funcs
        } else {
            &TabFuncs::new::<EmptyTabImpl>()
        };

        let tab_keybinds = (tab_funcs.keybinds_fn)();

        terminal.draw(|f| render_tui(context, f, &global_keybinds, &tab_keybinds)).map_err(|_| ())?;
        let ev = crossterm::event::read().map_err(|_| ())?;
        handle_event(context, ev, &global_keybinds, &tab_keybinds);
        if context.get_resource_mut::<Tui>().unwrap().should_quit() {
            break Ok(());
        }
    };
    ratatui::restore();
    result
}

fn render_tui(context: &mut Context, frame: &mut ratatui::Frame, keybinds: &Vec<KeyBind>, tab_keybinds: &Vec<KeyBind>) {
    // TODO: there has to be a better way to do this
    let mut tab_keybinds_copy = tab_keybinds.clone();
    let mut combined_keybinds = keybinds.clone();
    combined_keybinds.append(&mut tab_keybinds_copy);

    let mut keybind_lines = vec!(Vec::new());
    let mut width_so_far = 0;
    for keybind in &combined_keybinds {
       let text = keybind.display_text();
       if width_so_far + text.width() + 1 < frame.area().width as usize {
           width_so_far += text.width() + 1;
           keybind_lines.last_mut().unwrap().push(text);
       } else {
           width_so_far = text.width() + 1;
           keybind_lines.push(vec!(text));
       }
    };

    let frame_area = frame.area();

    frame.buffer_mut().set_style(frame_area, Style::default().bg(tailwind::SLATE.c800).fg(tailwind::SLATE.c100));

    let vertical_layout = Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(keybind_lines.len() as u16)]);
    let [header_area, content_area, footer_area] = vertical_layout.areas(frame.area());
    //frame.render_widget(Line::from("Training Assistant TUI").bold().centered(), header_area);
    render_tabs(context, frame);
    let selected_tab = context.get_resource_mut::<Tui>().unwrap().selected_tab;
    let tab_id = context.get_resource::<Tui>().unwrap().tabs[selected_tab].0;
    let tab_funcs = if let Some(funcs) = &context.get_resource_mut::<Tui>().unwrap().tabs[selected_tab].1.funcs {
        funcs
    } else {
        &TabFuncs::new::<EmptyTabImpl>()
    };
    (tab_funcs.render_fn)(context, frame.buffer_mut(), content_area, Block::new()
        .border_type(BorderType::QuadrantOutside).borders(Borders::ALL).bg(tailwind::SLATE.c900), tab_id);

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
        .map(|t| t.1.title.clone());
    Tabs::new(titles)
        .highlight_style(Style::new().bg(tailwind::RED.c700).fg(tailwind::RED.c100))
        .padding(" ", "")
        .divider(" ")
        .select(context.get_resource_mut::<Tui>().unwrap().selected_tab)
        .render(frame.area(), frame.buffer_mut());
}

fn event_to_key_bind(ev: crossterm::event::Event, keybinds: &Vec<KeyBind>) -> Option<&KeyBind> {
    let (key_code, modifiers) = match ev {
        Event::Key(key_event) => {
            if key_event.kind == KeyEventKind::Press {
                (key_event.code, key_event.modifiers)
            } else {
                return None;
            }
        },
        _ => { return None; }
    };

    keybinds.iter().find(|k| key_code == k.key_code && modifiers == k.modifiers)
}

fn handle_event(context: &mut Context, ev: crossterm::event::Event, global_keybinds: &Vec<KeyBind>, tab_keybinds: &Vec<KeyBind>) {
    if let Some(bind) = event_to_key_bind(ev.clone(), global_keybinds) {
        match bind.name.as_str() {
            "quit" => {
                context.get_resource_mut::<Tui>().unwrap().request_quit();
            },
            "prev_tab" => {
                context.get_resource_mut::<Tui>().unwrap().cycle_tab_prev();
            },
            "next_tab" => {
                context.get_resource_mut::<Tui>().unwrap().cycle_tab_next();
            },
            "new_tab" => {
                Tui::add_tab(Tab::new::<EmptyTabImpl>("New Tab"), context);
            },
            "close_tab" => {
                let selected_tab = context.get_resource_mut::<Tui>().unwrap().selected_tab;
                context.get_resource_mut::<Tui>().unwrap().tabs.remove(selected_tab);
            },
            "clear_tab" => {
                // TODO: fix this w/ tab ids
                let selected_tab = context.get_resource_mut::<Tui>().unwrap().selected_tab;
                let selected_tab_id = context.get_resource_mut::<Tui>().unwrap().tabs[selected_tab].0;
                let tab = Tab::new::<EmptyTabImpl>("New Tab");
                Tui::set_tab(selected_tab_id, tab, context); 
            }
            _ => { }
        }
    } else if let Some(bind) = event_to_key_bind(ev.clone(), tab_keybinds) {
        let selected_tab = context.get_resource_mut::<Tui>().unwrap().selected_tab;
        let funcs = context.get_resource_mut::<Tui>().unwrap().tabs[selected_tab].1.funcs.clone().unwrap_or(TabFuncs::new::<EmptyTabImpl>()).clone();
        (funcs.handle_key_fn)(context, bind.name.as_str(), selected_tab);
    }
}

fn process_tui_command(context: &mut Context, arg_matches: &ArgMatches) -> Result<CommandResponse> {
    context.add_resource(Tui::default());
    Tui::add_tab(Tab::new_empty("Empty Tab"), context);
    Ok(CommandResponse::new("Opening TUI session..."))
}

struct EmptyTabImpl;

impl TabImpl for EmptyTabImpl {
    type State = EmptyTabState;

    fn title() -> String {
        "Empty Tab".into()
    }

    fn render(context: &mut Context, buffer: &mut Buffer, rect: Rect, block: Block, tab_id: usize) {
        let tui_new_tab_types = context.get_resource_mut::<TuiNewTabTypes>().unwrap();
        let items = tui_new_tab_types.types.iter().map(|t| t.name.clone());
        if items.len() > 0 {
            let list = List::new(items)
                .block(block)
                .highlight_style(Style::new().fg(Color::Black).bg(Color::White))
                .highlight_symbol(">")
                .highlight_spacing(HighlightSpacing::Always);
            StatefulWidget::render(list, rect, buffer, &mut context.get_resource_mut::<TabState<EmptyTabState>>().expect("get_resource failed").get_state_mut(tab_id).unwrap().list_state);
        } else {
            Paragraph::new("No creatable tab types.").block(block).render(rect, buffer);
        }
    }

    fn keybinds() -> Vec<KeyBind> {
        vec![
            KeyBind {
                display_key: "Up".into(),
                display_name: "Move Up".into(),
                key_code: KeyCode::Up,
                name: "move_up".into(),
                modifiers: KeyModifiers::NONE,
            },
            KeyBind {
                display_key: "Down".into(),
                display_name: "Move Down".into(),
                key_code: KeyCode::Down,
                name: "move_down".into(),
                modifiers: KeyModifiers::NONE,
            },
            KeyBind {
                display_key: "Enter".into(),
                display_name: "Select".into(),
                key_code: KeyCode::Enter,
                name: "select".into(),
                modifiers: KeyModifiers::NONE,
            }
        ]
    }

    fn handle_key(context: &mut Context, bind: &str, tab_idx: usize) {
        match bind {
            "move_up" => {
                context.get_resource_mut::<TabState<EmptyTabState>>().unwrap().get_state_mut(tab_idx).unwrap().list_state.select_previous();
            },
            "move_down" => {
                context.get_resource_mut::<TabState<EmptyTabState>>().unwrap().get_state_mut(tab_idx).unwrap().list_state.select_next();
            },
            "select" => {
                let selected_new_tab = context.get_resource_mut::<TabState<EmptyTabState>>().unwrap().get_state(tab_idx).unwrap().list_state.selected().unwrap();
                let tab_funcs = context.get_resource_mut::<TuiNewTabTypes>().unwrap().types[selected_new_tab].funcs.clone();
                let selected_tab_idx = context.get_resource_mut::<Tui>().unwrap().selected_tab;
                let selected_tab_id = context.get_resource_mut::<Tui>().unwrap().tabs[selected_tab_idx].0;
                let tab = Tab {
                    title: "New Tab".into(),
                    funcs: Some(tab_funcs)
                };
                Tui::set_tab(selected_tab_id, tab, context); 
            }
            _ => { }
        }
    }
}

struct EmptyTabState {
    list_state: ListState    
}

impl Default for EmptyTabState {
    fn default() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            list_state
        }
    }
}

impl Resource for EmptyTabState {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
