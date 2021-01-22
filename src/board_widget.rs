use druid::{Widget, EventCtx, LifeCycle, PaintCtx, LifeCycleCtx, BoxConstraints, Size, LayoutCtx, Event, Env, UpdateCtx, Point, Rect, Color, Affine, MouseEvent, TextLayout, Selector, Target, KbKey};
use druid::RenderContext;
use druid::widget::{SvgData, Label};
use druid::kurbo::Circle;

use crate::State;
use std::fs::File;
use std::io::prelude::*;


use log::{debug, error};
use itertools::rev;
use chess::{Square, Piece, Board, ChessMove, MoveGen, BitBoard, Game};
use crate::uci::{Uci, Analysis};
use std::process::Command;
use std::collections::HashSet;
use std::thread;


const BROWN :Color = Color::rgb8(0x91, 0x67, 0x2c);
const WHITE :Color = Color::WHITE;
const HIGHLIGHT :Color = Color::AQUA;
const GREEN :Color = Color::GREEN;


pub struct BoardWidget {
    analysis_uci: Uci,   // keep the analysis with the widget
    square_size: f64,
    white_bottom: bool, // is white on the bottom of the board?
    mouse_down: Option<MouseEvent>, // we deal with mouse events on the _up_ or _move_, so just record this
    selected_square: Option<Square>,
    dragging_piece: Option<(Square, Point)>,  // square on the board being dragged & it's current position
    pieces_being_attacked: HashSet<Square>
}

impl BoardWidget {
    pub(crate) fn new() -> Self {
        // setup the stockfish engine
        let mut stockfish_cmd = Command::new("/usr/games/stockfish");

        BoardWidget {
            analysis_uci: Uci::start_engine(&mut stockfish_cmd),
            square_size: 0.0,
            white_bottom: true,
            mouse_down: None,
            selected_square: None,
            dragging_piece: None,
            pieces_being_attacked: HashSet::new()
        }
    }

    /// Converts a point on the board into a square
    fn point2square(&self, point :&Point) -> Square {
        let (row, col) = if self.white_bottom {
            (7 - ((point.y / self.square_size) as u8), (point.x / self.square_size) as u8)
        } else {
            ((point.y / self.square_size) as u8, 7 - ((point.x / self.square_size) as u8))
        };

        // debug!("P:{} -> R:{} C:{}", point, row, col);

        unsafe { Square::new(8 * row + col) }
    }

    /// Gets the rectangle bounding a given square
    fn square2rect(&self, square :&Square) -> Rect {
        // the origin is in the upper-left corner of the drawing area
        // compute the row & col based upon if the board is flipped or not
        let (row, col) = if self.white_bottom {
            ((7-square.get_rank().to_index()), square.get_file().to_index())
        } else {
            (square.get_rank().to_index(), (7-square.get_file().to_index()))
        };

        let point = Point::new(self.square_size * col as f64, self.square_size * row as f64);
        let rect = Rect::from_origin_size(point, Size::new(self.square_size, self.square_size));

        // debug!("R: {} C: {} => {}", row, col, rect);

        rect
    }

    fn square2piece(board: &Board, square: &Square) -> Option<(Piece, chess::Color)> {
        if let Some(piece) = board.piece_on(square.clone()) {
            let color = board.color_on(square.clone()).unwrap();

            Some( (piece, color) )
        }
        else {
            None
        }
    }

    fn square2svg(board: &Board, square: &Square) -> Option<SvgData> {
        if let Some( (piece, color) ) = BoardWidget::square2piece(board, square) {
            // debug!("{:?} => {:?}", square, piece);

            // TODO: Save this data so we're not opening & reading files every time the board is drawn
            let mut file = File::open(format!("/home/wspeirs/src/cgir/src/assets/svg/{}.svg", piece.to_string(color))).unwrap();
            let mut contents = String::new();
            file.read_to_string(&mut contents).unwrap();

            Some(contents.parse::<SvgData>().unwrap())
        } else {
            None
        }
    }
}

impl Widget<State> for BoardWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut State, env: &Env) {
        // debug!("Board::event: {:?}", event);

        match event {
            Event::KeyUp(key_event) => {
                debug!("KEY UP: {:?}", key_event.key);

                if let KbKey::Escape = key_event.key {
                    self.mouse_down = None;
                    self.selected_square = None;
                    self.dragging_piece = None;
                    ctx.set_handled();
                    ctx.request_paint();
                }
            },
            Event::MouseDown(mouse_event) => { self.mouse_down = Some(mouse_event.clone()); },
            Event::MouseUp(mouse_event) => {
                debug!("MOUSE UP");
                // first check to see if we have a MouseDown... if not, that's an error
                if self.mouse_down.is_none() {
                    panic!("No corresponding MouseDown event");
                }

                // convert both the down & up to squares
                let down_square = self.point2square(&self.mouse_down.as_ref().unwrap().pos);
                let up_square = self.point2square(&mouse_event.pos);

                debug!("DOWN ON: {} UP ON: {}", down_square, up_square);

                // reset the drag and mouse down regardless
                self.dragging_piece = None;
                self.mouse_down = None;

                let mut chess_move = None;

                // we're moving here
                if down_square != up_square {
                    let color_to_move = data.game.current_position().side_to_move();
                    let (piece, color) = Self::square2piece(&data.game.current_position(), &down_square).unwrap();

                    // make sure the correct side is trying to move
                    if color_to_move != color {
                        error!("Trying to move the wrong color");
                        return
                    }

                    // generate the target move the player is trying to make
                    let target_move = ChessMove::new(down_square, up_square, None);

                    // generate the legal moves that land on the to_square
                    let mut moves = MoveGen::new_legal(&data.game.current_position());
                    moves.set_iterator_mask(BitBoard::from_square(up_square));

                    for m in &mut moves {
                        // we found the move as a legal move
                        if m == target_move {
                            chess_move = Some(target_move); // save the move to make
                            self.selected_square = None; // remove anything that was selected
                            break
                        }
                    }
                } else {
                    // check to see if we already have a piece selected
                    if let Some(selected_square) = self.selected_square {
                        // 3 options here:
                        // 2) clicking on the selected square... so deselect it
                        // 1) clicking on a square w/out a piece, so have to make sure it's a legal move
                        // 3) clicking on another one of their pieces, select _that_ one

                        // check to see if they clicked the _same_ square
                        if selected_square == up_square {
                            debug!("SAME SQUARE");
                            self.selected_square = None;
                            self.mouse_down = None;
                        } else {
                            let op_piece = Self::square2piece(&data.game.current_position(), &down_square);

                            if let Some((_piece, color)) = op_piece {
                                if color == data.game.current_position().side_to_move() {
                                    self.selected_square = Some(down_square);
                                } else {
                                    // we use this as a flag below
                                    self.selected_square = None;
                                }
                            }

                            if op_piece.is_none() || self.selected_square.is_none() {
                                // need to find all the legal moves for this piece, and mark those squares
                                let moves = MoveGen::new_legal(&data.game.current_position());

                                for m in moves {
                                    // skip moves that don't originate on the selected square
                                    if m.get_source() != selected_square {
                                        continue
                                    }

                                    // a legal move is the same as the square that was clicked
                                    if m.get_dest() == down_square {
                                        // set the move
                                        chess_move = Some(m);
                                        debug!("GOT LEGAL MOVE: {:?}", chess_move);
                                        break
                                    }
                                }
                            }
                        }

                        ctx.request_paint(); // request a re-paint
                    } else {
                        // we don't already have a square selected
                        // so check if it's on a square with a piece
                        if let Some((_piece, color)) = Self::square2piece(&data.game.current_position(), &down_square) {
                            // and that color is the one to move
                            if color == data.game.current_position().side_to_move() {
                                self.selected_square = Some(down_square);
                                ctx.request_paint(); // request a re-paint
                            }
                        }
                    }
                }

                // check to see if we have a move to make
                if let Some(mv) = chess_move {
                    // make the move in the game
                    data.game.make_move(mv);

                    // start the computer's analysis
                    let rx = data.engine.analyze(&data.game, Some(7));
                    let event_sink = ctx.get_external_handle();

                    // spawn a thread to report back when the move has been made
                    thread::spawn(move || {
                        for analysis in rx.iter() {
                            println!("ANALYSIS: {:?}", analysis);

                            // if we get the best move, then send it as an event
                            if let Analysis::BestMove(best_move) = analysis {
                                if let Err(e) = event_sink.submit_command(Selector::<ChessMove>::new("best_move"), Box::new(best_move), Target::Global) {
                                    error!("Error submitting best-move: {:?}", e);
                                }
                            }
                        }
                    });

                    // make sure nothing is selected
                    self.selected_square = None;
                }
            }, // end of MouseUp match
            Event::MouseMove(mouse_event) => {
                // we want to register this widget for focus if the mouse is over it
                if !ctx.has_focus() {
                    ctx.request_focus();
                }

                // make sure the user is attempting to drag a piece
                if self.mouse_down.is_none() {
                    return;
                }

                // debug!("DRAGGING");

                // check to see if we already know we're dragging
                if let Some((square, pos)) = self.dragging_piece.as_mut() {
                    // just update the position
                    *pos = mouse_event.pos;
                } else {
                    // this is a new drag, so set it up
                    let down_square = self.point2square(&self.mouse_down.as_ref().unwrap().pos);

                    // see if there is a piece associated with the MouseDown event
                    if let Some((piece, color)) = Self::square2piece(&data.game.current_position(), &down_square) {
                        // make sure it's the right color (do we really care?!?)
                        if color != data.game.current_position().side_to_move() {
                            return; // just bail
                        }
                        self.dragging_piece = Some( (down_square, mouse_event.pos) );
                    }
                }

                // call paint to update
                ctx.request_paint();
            },
            Event::Command(cmd) => {
                // check to see if we got a best move from the computer
                if let Some(best_move) = cmd.get(Selector::<ChessMove>::new("best_move")) {
                    println!("GOT BEST MOVE: {:?}", best_move);

                    // make the move
                    data.game.make_move(*best_move);

                    // request an update
                    ctx.request_update();

                    // mark the event as handled
                    ctx.set_handled();
                } else if let Some(()) = cmd.get(Selector::<()>::new("update")) {
                    ctx.request_update();

                    // mark the event as handled
                    ctx.set_handled();
                }
            }
            _ => { }
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &State, env: &Env) {
        debug!("Board::lifecycle: {:?}", event);

        if let LifeCycle::WidgetAdded = event {
            ctx.register_for_focus();
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &State, data: &State, env: &Env) {
        debug!("Widget::update");

        // check for all our pieces being attacked
        let mut white_board = data.game.current_position().color_combined(chess::Color::White).clone();
        let white_squares = white_board.into_iter().collect::<HashSet<Square>>();

        // get the board, and reset our set
        let board = data.game.current_position();
        self.pieces_being_attacked.clear();

        // if we're supposed to show the pieces being attacked, compute them
        if data.show_pieces_being_attacked {
            // go through all the white squares
            for ws in white_squares {
                // get all of the bishop, rook, and queen attackers for black
                let attackers = board.color_combined(chess::Color::Black) &
                    ((chess::get_bishop_rays(ws) & (board.pieces(Piece::Bishop) | board.pieces(Piece::Queen))) |
                        (chess::get_rook_rays(ws) & (board.pieces(Piece::Rook) | board.pieces(Piece::Queen))));

                for attack_square in attackers {
                    let between = chess::between(ws, attack_square) & board.combined();

                    // if nothing is between these two squares, then it's an attack
                    if between == chess::EMPTY {
                        println!("{} ATTACKING {}", attack_square, ws);
                        self.pieces_being_attacked.insert(ws);
                    }
                }

                // now look at the knights
                let attackers = chess::get_knight_moves(ws) & board.color_combined(chess::Color::Black) & board.pieces(Piece::Knight);

                for attack_square in attackers {
                    println!("KNIGHT {} ATTACKING {}", attack_square, ws);
                    self.pieces_being_attacked.insert(ws);
                }
            }

            // now look at pawn attacks
            for black_pawn_square in board.color_combined(chess::Color::Black) & board.pieces(Piece::Pawn) {
                let attackers = chess::get_pawn_attacks(black_pawn_square, chess::Color::Black, *board.color_combined(chess::Color::White));

                for attacked_square in attackers {
                    println!("PAWN ATTACKING {}", attacked_square);
                    self.pieces_being_attacked.insert(attacked_square);
                }
            }
        }


        // // do the analysis
        // let analysis = self.uci.analyze(&data.game, Some(5));
        //
        // for a in analysis.iter() {
        //     println!("GOT: {:?}", a);
        // }

        // just always request a paint
        ctx.request_paint();
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &State, env: &Env) -> Size {
        debug!("Board::layout: {:?}", bc);

        let max_size = bc.max();
        let min_side = max_size.height.min(max_size.width);

        // save off the size of the square
        self.square_size = min_side / 8.0f64;

        debug!("SQUARE SIZE: {:?}", self.square_size);

        // return something that's square
        Size {
            width: min_side,
            height: min_side,
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &State, env: &Env) {
        // debug!("Board::paint");

        // compute the scale ratio between the space size and the piece
        let svg_scale = Affine::scale(self.square_size / 45.0f64);

        // go through each square on the board painting it
        let sq_it :Box<dyn Iterator<Item=&Square>> = if !self.white_bottom { Box::new(chess::ALL_SQUARES.iter().rev()) } else { Box::new(chess::ALL_SQUARES.iter()) };

        for square in sq_it {
            // debug!("SQ: {}", square);
            let rect = self.square2rect(square);

            let env_clone = env.clone();

            // this paints the colored square
            let square_color = if self.pieces_being_attacked.contains(square) {
                Color::RED
            } else {
                // this is convoluted, but works :-)
                if ((square.get_rank().to_index() % 2) + square.get_file().to_index()) % 2 == 0 {
                    BROWN
                } else {
                    WHITE
                }
            };

            ctx.paint_with_z_index(1, move |ctx| {
                ctx.fill(rect, &square_color);
            });

            // label the squares on top of their color
            ctx.paint_with_z_index(2, move |ctx| {
                let mut label = TextLayout::<String>::from_text(format!("{}", square));
                label.set_text_color(Color::BLACK);
                label.set_text_size(10.5);

                label.rebuild_if_needed(ctx.text(), &env_clone);
                label.draw(ctx, Point::new(rect.x0, rect.y0));
            });

            // first check to see if we're dragging a piece
            if let Some((dragging_square, pos)) = self.dragging_piece {
                // ... and the current square is the one being dragged
                if *square == dragging_square {
                    let piece_svg = Self::square2svg(&data.game.current_position(), &dragging_square).unwrap();

                    // paint this piece in the middle of the mouse position
                    ctx.paint_with_z_index(3, move |ctx| {
                        // translate to the position of the mouse, minus half the size of the image
                        let translate = Affine::translate((pos.x-22.5, pos.y-22.5) );
                        piece_svg.to_piet(translate * svg_scale.clone(), ctx);
                    });

                    continue
                }
            }

            // check to see if we have a piece on this square
            if let Some(piece_svg) = Self::square2svg(&data.game.current_position(), &square) {
                // we want our pieces on top of our squares
                ctx.paint_with_z_index(2, move |ctx| {
                    let translate = Affine::translate((rect.min_x(), rect.min_y()) );

                    // debug!("RECT: {:?} -> ({}, {})", rect, rect.min_y(), rect.min_x());

                    // we have to do translate * svg_scale (not svg_scale * translate)
                    // otherwise, our translate is "in" the scale
                    piece_svg.to_piet(translate * svg_scale.clone(), ctx);
                });
            }
        }

        // check to see if we have a selected square
        if let Some(selected_square) = self.selected_square {
            debug!("SELECTED: {:?}", selected_square);

            let rect = self.square2rect(&selected_square);

            // paint it with the highlight color
            ctx.paint_with_z_index(1, move |ctx| {
                ctx.fill(rect, &HIGHLIGHT);
            });

            // need to find all the legal moves for this piece, and mark those squares
            let moves = MoveGen::new_legal(&data.game.current_position());

            debug!("MOVES: {}", moves.len());

            for m in moves {
                // skip moves that don't originate from this square
                if m.get_source() != selected_square {
                    continue;
                }

                let dest_rect = self.square2rect(&m.get_dest());
                let radius = dest_rect.width() * 0.165f64; // cover 33% of the square
                let dot = Circle::new(Point::new(dest_rect.min_x() + dest_rect.width()/2.0, dest_rect.min_y() + dest_rect.width()/2.0), radius);

                debug!("VALID: {:?} -> {:?}", m.get_dest(), dot);

                ctx.paint_with_z_index(3, move |ctx| {
                    ctx.fill(dot, &GREEN);
                });
            }
        }
    }

    fn type_name(&self) -> &'static str {
        "board"
    }
}