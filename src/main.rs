use druid::widget::prelude::*;
use druid::widget::{Align, BackgroundBrush, Button, Controller, ControllerHost, Flex, Label, Padding, Container, Split};
use druid::Target::Global;
use druid::{
    commands as sys_cmds, AppDelegate, AppLauncher, Application, Color, Command, ContextMenu, Data,
    DelegateCtx, Handled, LocalizedString, MenuDesc, MenuItem, Selector, Target, WindowDesc,
    WindowId,
};

use log::info;

mod board;

use board::Board;

#[derive(Debug, Clone, Default, Data)]
pub struct State {
}

pub fn main() {
    let main_window = WindowDesc::new(ui_builder)
        .menu(make_menu(&State::default()))
        .title("CGIR - Chess GUI in Rust");

    AppLauncher::with_window(main_window)
        .use_simple_logger()
        .launch(State::default())
        .expect("launch failed");
}

fn ui_builder() -> impl Widget<State> {
    let top_container = Container::new(
        Split::columns(
            Align::centered(Board{}),
            Align::centered(Label::new("PLYS"))
        ).draggable(true)
    );

    let window_container = Container::new(
        Split::rows(
            Align::centered(top_container),
            Align::centered(Label::new("ANALYSIS"))
        ).draggable(true)
    );

    window_container
}

#[allow(unused_assignments)]
fn make_menu<T: Data>(_state: &State) -> MenuDesc<T> {
    let mut base = MenuDesc::empty();
    #[cfg(target_os = "macos")]
    {
        base = druid::platform_menus::mac::menu_bar();
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        base = base.append(druid::platform_menus::win::file::default());
    }

    base
}

