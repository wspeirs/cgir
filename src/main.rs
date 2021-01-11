use std::collections::HashSet;
use std::default::Default;

use druid::widget::prelude::*;
use druid::widget::{Align, BackgroundBrush, Button, Controller, ControllerHost, Flex, Label, Padding, Container, Split, SvgData, Svg, List, Scroll};
use druid::Target::Global;
use druid::{commands as sys_cmds, AppDelegate, AppLauncher, Application, Color, Command, ContextMenu, Data, DelegateCtx, Handled, LocalizedString, MenuDesc, MenuItem, Selector, Target, WindowDesc, WindowId, WidgetExt, MouseEvent, WindowState, UnitPoint};

use log::info;
use chess::Board;

mod board_widget;

use board_widget::BoardWidget;

#[derive(Debug, Clone, Default)]
pub struct State {
    board: Board, // this is our chess board,
}

impl Data for State {
    fn same(&self, other: &Self) -> bool {
        self.board.combined() == other.board.combined()
    }
}

pub fn main() {
    let main_window = WindowDesc::new(ui_builder)
        .set_window_state(WindowState::MAXIMIZED)
        .window_size(Size::new(1024.0, 1024.0))
        .menu(make_menu(&State::default()))
        .title("CGIR - Chess GUI in Rust");

    AppLauncher::with_window(main_window)
        .use_simple_logger()
        .launch(State::default())
        .expect("launch failed");
}

fn ui_builder() -> impl Widget<State> {
    // let ply_list = Scroll::new(List::new(|| {
    //     Label::new(|item: &u32, _env: &_| format!("List item #{}", item))
    //         .align_vertical(UnitPoint::LEFT)
    //         .padding(10.0)
    //         .expand()
    //         .height(50.0)
    //         .background(Color::rgb(0.5, 0.5, 0.5))
    //     })
    // );


    // this holds the top 2 splits: board | Plys
    let top_container = Container::new(
        Split::columns(
            Align::centered(BoardWidget::new()),
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

