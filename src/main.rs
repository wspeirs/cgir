use std::default::Default;

use druid::widget::prelude::*;
use druid::widget::{Align, Flex, Label, Container, Split, List, Scroll, Controller, Button};
use druid::{AppLauncher, Color, Data, MenuDesc, MenuItem, WindowDesc, WidgetExt, WindowState, Lens, UnitPoint, Selector, Target};

// use log::{debug, info};
use chess::{Game, Action};

mod board_widget;
mod uci;
mod chess_utils;

use board_widget::BoardWidget;
use druid::im::Vector;
use std::process::{Command, Stdio};
use crate::uci::Uci;
use std::sync::Arc;


#[derive(Debug, Clone)]
pub struct State {
    game: Game,     // state of our chess game
    engine: Uci,    // engine the human is playing against
    show_pieces_being_attacked: bool,  // should we show pieces being attacked
}

impl Data for State {
    fn same(&self, other: &Self) -> bool {
        self.game.current_position().combined() == other.game.current_position().combined()
    }
}

impl State {
    fn new() -> Self {
        // setup an engine to play against
        let mut engine_cmd = Command::new("/usr/games/ethereal-chess");
        let engine = Uci::start_engine(&mut engine_cmd);

        State {
            game: Game::new(),
            engine,
            show_pieces_being_attacked: true
        }
    }
}

struct MoveList;

impl Lens<State, Vector<String>> for MoveList {
    fn with<V, F: FnOnce(&Vector<String>) -> V>(&self, data: &State, f: F) -> V {
        // convert the list of actions into strings
        // TODO: add move numbers as well
        let move_list :Vector<String> = data.game.actions().chunks(2).enumerate().map(|(num, actions)| {
            let a1 = match actions[0] {
                Action::MakeMove(chess_move) => { format!("{}: {}", num+1, chess_move)}
                Action::Resign(color) => { format!("{}: {:?} resigns", num+1, color)}
                _ => unimplemented!("Cannot convert draws to moves")
            };

            if actions.len() == 2 {
                let a2 = match actions[1] {
                    Action::MakeMove(chess_move) => { format!("{}", chess_move)}
                    Action::Resign(color) => { format!("{:?} resigns", color)}
                    _ => unimplemented!("Cannot convert draws to moves")
                };

                format!("{} {}", a1, a2)
            } else {
                a1
            }
        }).collect();

        f(&move_list)
    }

    fn with_mut<V, F: FnOnce(&mut Vector<String>) -> V>(&self, data: &mut State, f: F) -> V {
        f(&mut Vector::new())
    }
}

pub fn main() {
    // create a default state
    let state = State::new();

    let main_window = WindowDesc::new(ui_builder)
        .set_window_state(WindowState::MAXIMIZED)
        .window_size(Size::new(1024.0, 1024.0))
        .menu(make_menu(&state))
        .title("CGIR - Chess GUI in Rust");

    AppLauncher::with_window(main_window)
        .use_simple_logger()
        .launch(state)
        .expect("launch failed");
}

fn ui_builder() -> impl Widget<State> {
    let ply_list = Scroll::new(List::new(|| {
        Label::new(|chess_move :&String, _env: &_| chess_move.clone())
            .align_vertical(UnitPoint::LEFT)
            .padding(7.0)
            .expand()
            .height(25.0)
            .background(Color::BLACK)
    }).lens(MoveList))
        .vertical()
        .align_vertical(UnitPoint::TOP_LEFT)
        ;

    let bw = BoardWidget::new();

    // this holds the top 2 splits: board | Plys
    let top_container = Container::new(
        Split::columns(
            Align::centered(bw),
            Align::centered(ply_list)
        ).draggable(true)
    );

    let attacker_button = Button::new("Show Attackers").on_click(move |ctx, data: &mut State, env| {
        println!("SHOW: {}", data.show_pieces_being_attacked);

        data.show_pieces_being_attacked ^= true;
        ctx.get_external_handle().submit_command(Selector::<()>::new("update"), Box::new(()), Target::Global);
    });

    let window_container = Container::new(
        Split::rows(
            Align::centered(top_container),
            Align::centered(attacker_button)
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

