//! A common terminal UI implementation.
use framework::prelude::*;
use std::any::Any;
use ratatui::layout::{Rect, Layout, Constraint};
use crossterm::event::{Event, KeyModifiers, KeyEventKind, KeyCode};
use ratatui::style::{Stylize, Style, Color, palette::tailwind};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph, StatefulWidget, HighlightSpacing, Borders, Tabs, Widget, List, ListState};
use ratatui::buffer::Buffer;
use clap::{ArgMatches, Command};
use std::collections::HashMap;

/// The plugin for the TUI system.
/// Add this to set up the required resources and commands.
#[derive(Clone)]
pub struct TuiPlugin;

impl Plugin for TuiPlugin {
    fn build(self, context: &mut Context) -> Result<()> {
        context.add_resource(TuiNewTabTypes::default());
        context.add_command(
            Command::new("tui")
                .about("Opens an empty TUI session."),
            process_tui_command
        )?;
        Ok(())
    }
}

/// A `Resource` that stores information about the TUI state.
#[derive(Default)]
pub struct Tui {
    quit_requested: bool,
    tabs: Vec<(usize, Tab)>,
    selected_tab: usize,
    next_tab_id: usize
}

impl Tui {
    /// Call this to request that the process running this TUI should exit.
    pub fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    /// Returns whether or not the TUI session should exit.
    pub fn should_quit(&self) -> bool {
        self.quit_requested
    }

    fn cycle_tab_next(&mut self) {
        self.selected_tab = (self.selected_tab + 1) % self.tabs.len();
    }

    fn cycle_tab_prev(&mut self) {
        self.selected_tab = (self.selected_tab - 1) % self.tabs.len();
    }

    /// Adds a tab to the TUI session.
    ///
    /// * `tab` - The tab to add.
    /// * `context` - The context running the TUI session.
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


    /// Sets a tab's content to the given `Tab`.
    ///
    /// * `tab_id` - The tab ID to set.
    /// * `tab` - The tab content the tab should have.
    /// * `context` - The context running the TUI session.
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

/// A resource storing the tab types that can be created in the new tab UI.
#[derive(Default)]
pub struct TuiNewTabTypes {
    types: Vec<TuiNewTabType>
}

impl TuiNewTabTypes {
    /// Registers a tab type (`T`) to be included in the new tab UI.
    ///
    /// * `name` - The name to display for the option in the new tab UI.
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

/// A resource for storing an associated state for a tab.
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

    /// Gets a mutable reference to the state associated to a given tab ID.
    /// Returns `None` if the tab ID doesn't have a registered state of this type.
    ///
    /// * `tab_id` - The tab ID to get the state for.
    pub fn get_state_mut(&mut self, tab_id: usize) -> Option<&mut T> {
        self.states.get_mut(&tab_id)
    }

    /// Gets a reference to the state associated to a given tab ID.
    /// Returns `None` if the tab ID doesn't have a registered state of this type.
    ///
    /// * `tab_id` - The tab ID to get the state for.
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

/// Stores data used by the TUI system to manage a single tab.
pub struct Tab {
    funcs: Option<TabFuncs>,
}

impl Tab {
    /// Creates a new `Tab` with the given implementation type (`T`) and the given title.
    pub fn new<T>() -> Self 
        where T : TabImpl
    {
        Self {
            funcs: Some(TabFuncs::new::<T>())
        }
    }

    /// Creates a new `Tab` with no content. This will use `EmptyTabImpl`.
    pub fn new_empty() -> Self {
        Self {
            funcs: None
        }
    }
}

/// A trait for tab operations for a particular tab type.
pub trait TabImpl {
    /// The state type to create (accessible through the `TabState<T>` resource)
    type State: Default + 'static;

    /// Gets the title of the tab.
    fn title() -> String;

    /// Renders a tab to a TUI buffer.
    ///
    /// * `context` - The `Context` running the TUI session.
    /// * `buffer` - The `Buffer` to render into.
    /// * `area` - The `Rect` representing the area of the `Buffer` to render into.
    /// * `block` - The `Block` containing the tab content.
    /// * `tab_id` - The tab ID of the tab.
    // TODO: this should return Result<()>
    fn render(context: &mut Context, buffer: &mut Buffer, area: Rect, block: Block, tab_id: usize);

    /// Gets the keybinds used by the tab. These will be displayed at the bottom of the TUI.
    fn keybinds() -> Vec<KeyBind>;

    /// Handles a key event. Only called for keybinds returned by `Self::keybinds`.
    ///
    /// * `context` - The `Context` running the TUI session.
    /// * `bind_name` - The name of the keybind pressed.
    /// * `tab_idx` - The tab ID of the currently focused tab.
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

/// A single keybind, associating together an internal name, display name, key code, and modifiers.
#[derive(Clone)]
pub struct KeyBind {
    /// The internal name to use to denote the bind.
    pub name: String,

    /// The text to display representing the key (combination).
    pub display_key: String,

    /// The text to display representing the key action.
    pub display_name: String,
    
    /// The code of the key to bind.
    pub key_code: KeyCode,

    /// Additional modifiers (ctrl, alt, shift, etc)
    pub modifiers: KeyModifiers,
}

impl KeyBind {
    fn display_text(&self) -> Line<'_> {
        Line::from(vec![
           Span::styled(format!("<{}>", self.display_key), Style::default().fg(Color::Black).bg(Color::White)),
           Span::styled(format!(" {}", self.display_name), Style::default())
        ])
    }
}

fn global_keybinds() -> Vec<KeyBind> {
    vec!(
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
    )
}    

fn get_selected_tab_keybinds(context: &mut Context) -> Vec<KeyBind> {
    let selected_tab = context.get_resource_mut::<Tui>().unwrap().selected_tab;
    let tab_funcs = if let Some(funcs) = &context.get_resource_mut::<Tui>().unwrap().tabs[selected_tab].1.funcs {
        funcs
    } else {
        &TabFuncs::new::<EmptyTabImpl>()
    };

    (tab_funcs.keybinds_fn)()

}

// TODO: use a better result type
/// Runs a TUI session in the terminal.
///
/// * `context` - The `Context` running the TUI session.
pub fn run_tui(context: &mut Context) -> std::result::Result<(), ()> { 
    let mut terminal = ratatui::init();
    let result = loop {
        draw_tui(context, &mut terminal)?;
        let ev = crossterm::event::read().map_err(|_| ())?;
        handle_event(context, ev);
        if context.get_resource_mut::<Tui>().unwrap().should_quit() {
            break Ok(());
        }
    };
    ratatui::restore();
    result
}

/// Draws a frame of the TUI in the given `ratatui::Terminal`.
///
/// * `context` - The `Context` running the TUI.
/// * `terminal` - The `ratatui::Terminal` to draw into.
// TODO: result type is incorrect
pub fn draw_tui<B>(context: &mut Context, terminal: &mut ratatui::Terminal<B>) -> std::result::Result<(), ()> 
    where B: ratatui::backend::Backend
{
    // TODO: this error handling is incorrect
    terminal.draw(|f| render_tui(context, f)).map_err(|_| ())?;
    Ok(()) 
}

fn render_tui(context: &mut Context, frame: &mut ratatui::Frame) {
    let global_keybinds = global_keybinds();
    let mut tab_keybinds = get_selected_tab_keybinds(context);
    let mut combined_keybinds = global_keybinds.clone();
    combined_keybinds.append(&mut tab_keybinds);

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
    let [_header_area, content_area, footer_area] = vertical_layout.areas(frame.area());
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
        for i in 0..keybind_line.len() {
            frame.render_widget(Line::from(keybind_line[i].clone()), horizontal_layout[i]);
        }
    }
}

fn render_tabs(context: &mut Context, frame: &mut ratatui::Frame) {
    let titles = context.get_resource_mut::<Tui>().unwrap().tabs.iter()
        .map(|t| (t.1.funcs.clone().unwrap_or(TabFuncs::new::<EmptyTabImpl>()).title_fn)().clone());
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

fn handle_event(context: &mut Context, ev: crossterm::event::Event) {
    let global_keybinds = global_keybinds();
    let tab_keybinds = get_selected_tab_keybinds(context);
    if let Some(bind) = event_to_key_bind(ev.clone(), &global_keybinds) {
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
                Tui::add_tab(Tab::new::<EmptyTabImpl>(), context);
            },
            "close_tab" => {
                let selected_tab = context.get_resource_mut::<Tui>().unwrap().selected_tab;
                context.get_resource_mut::<Tui>().unwrap().tabs.remove(selected_tab);
            },
            "clear_tab" => {
                // TODO: fix this w/ tab ids
                let selected_tab = context.get_resource_mut::<Tui>().unwrap().selected_tab;
                let selected_tab_id = context.get_resource_mut::<Tui>().unwrap().tabs[selected_tab].0;
                let tab = Tab::new::<EmptyTabImpl>();
                Tui::set_tab(selected_tab_id, tab, context); 
            }
            _ => { }
        }
    } else if let Some(bind) = event_to_key_bind(ev.clone(), &tab_keybinds) {
        let selected_tab = context.get_resource_mut::<Tui>().unwrap().selected_tab;
        let funcs = context.get_resource_mut::<Tui>().unwrap().tabs[selected_tab].1.funcs.clone().unwrap_or(TabFuncs::new::<EmptyTabImpl>()).clone();
        (funcs.handle_key_fn)(context, bind.name.as_str(), selected_tab);
    }
}

fn process_tui_command(context: &mut Context, _: &ArgMatches) -> Result<CommandResponse> {
    context.add_resource(Tui::default());
    Tui::add_tab(Tab::new_empty(), context);
    Ok(CommandResponse::new("Opening TUI session..."))
}

struct EmptyTabImpl;

impl TabImpl for EmptyTabImpl {
    type State = EmptyTabState;

    fn title() -> String {
        "🔆 Empty Tab".into()
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

/// Common imports for working with the `tui` module.
pub mod prelude {
    pub use crate::{
        TuiPlugin, Tui, TabImpl, KeyBind, TuiNewTabTypes
    };

    pub use ratatui::{
        Frame,
        buffer::Buffer,
        layout::Rect,
        widgets::{Block, Paragraph, Widget},
        text::Line,
    };
}

#[cfg(test)]
mod test {
    use framework::prelude::*;
    use crate::prelude::*;

    #[test]
    fn tui_test_1() -> Result<()> {
        let mut context = Context::new();
        context.add_plugin(TuiPlugin)?;
        context.startup()?;
        let response = context.execute("tui")?;
        assert!(context.has_resource::<Tui>());
        assert!(response.text().is_some());
        assert_eq!(response.text().unwrap(), "Opening TUI session...");
        let backend = ratatui::backend::TestBackend::new(32, 16);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        crate::draw_tui(&mut context, &mut terminal).unwrap(); 
        insta::assert_snapshot!(terminal.backend());
        // TODO: get rid of the namespace spam here
        // create a new tab
        crate::handle_event(&mut context, 
            crossterm::event::Event::Key(crossterm::event::KeyEvent {
               code: crossterm::event::KeyCode::Char('t'),
               kind: crossterm::event::KeyEventKind::Press,
               state: crossterm::event::KeyEventState::empty(),
               modifiers: crossterm::event::KeyModifiers::CONTROL
            })
        );
        crate::draw_tui(&mut context, &mut terminal).unwrap(); 
        insta::assert_snapshot!(terminal.backend());
        // request quit
        crate::handle_event(&mut context, 
            crossterm::event::Event::Key(crossterm::event::KeyEvent {
               code: crossterm::event::KeyCode::Char('q'),
               kind: crossterm::event::KeyEventKind::Press,
               state: crossterm::event::KeyEventState::empty(),
               modifiers: crossterm::event::KeyModifiers::NONE
            })
        );
        assert!(context.get_resource::<Tui>().unwrap().should_quit());
        Ok(())
    }
}
