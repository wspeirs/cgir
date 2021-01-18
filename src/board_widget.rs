use druid::{Widget,
            EventCtx,
            LifeCycle,
            PaintCtx,
            LifeCycleCtx,
            BoxConstraints,
            Size,
            LayoutCtx,
            Event,
            Env,
            UpdateCtx,
            Point,
            Rect,
            Color,
            Affine,
            MouseEvent,
            TextLayout};
use druid::RenderContext;
use druid::widget::{Svg, SvgData, Label};
use druid::kurbo::Circle;

use crate::State;
use std::fs::File;
use std::io::prelude::*;


use log::{debug, error};
use chess::{Square, Piece, Board, ChessMove, MoveGen, BitBoard, Game};

const BROWN :Color = Color::rgb8(0x91, 0x67, 0x2c);
const WHITE :Color = Color::WHITE;
const HIGHLIGHT :Color = Color::AQUA;
const GREEN :Color = Color::GREEN;

pub struct BoardWidget {
    square_size: Size,
    mouse_down: Option<MouseEvent>, // we deal with mouse events on the _up_ or _move_, so just record this
    selected_square: Option<Square>,
    dragging_piece: Option<(Square, Point)>  // square on the board being dragged & it's current position
}

impl BoardWidget {
    pub(crate) fn new() -> Self {
        BoardWidget {
            square_size: Size::new(0.0, 0.0),
            mouse_down: None,
            selected_square: None,
            dragging_piece: None
        }
    }

    /// Converts a point on the board into a square
    fn point2square(&self, point :&Point) -> Square {
        // compute the row & col
        let row = (point.y / self.square_size.height) as u8;
        let col = (point.x / self.square_size.width) as u8;

        unsafe { Square::new(8 * row + col) }
    }

    /// Gets the rectangle bounding a given square
    fn square2rect(&self, square :&Square) -> Rect {
        let col = square.get_rank().to_index();
        let row = square.get_file().to_index();

        let point = Point {
            x: self.square_size.width * row as f64,
            y: self.square_size.height * col as f64,
        };

        Rect::from_origin_size(point, self.square_size)
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

                debug!("DOWN ON: {:?} UP ON: {:?}", down_square, up_square);

                // reset the drag and mouse down regardless
                self.dragging_piece = None;
                self.mouse_down = None;

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
                        // we found the move as a legal move, so update everything
                        if m == target_move {
                            data.game.make_move(target_move);
                            self.selected_square = None; // remove anything that was selected
                            return
                        }
                    }

                    // if we got here, it wasn't a legal move
                    error!("NOT A LEGAL MOVE: {:?}", target_move);
                } else {
                    // check to see if we already have a piece selected
                    // if we do, then they're trying to move that piece
                    if let Some(selected_square) = self.selected_square {
                        // need to find all the legal moves for this piece, and mark those squares
                        let moves = MoveGen::new_legal(&data.game.current_position());

                        for m in moves {
                            // skip moves that don't originate on the selected square
                            if m.get_source() != selected_square {
                                continue
                            }

                            // a legal move is the same as the square that was clicked
                            if m.get_dest() == down_square {
                                // make the move
                                data.game.make_move(m);
                                self.selected_square = None;
                                return
                            }
                        }

                        // if we got here, they tried to make an illegal move, so just unselect
                        self.selected_square = None;
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
            }, // end of MouseUp match
            Event::MouseMove(mouse_event) => {
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
            }
            _ => { }
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &State, env: &Env) {
        debug!("Board::lifecycle: {:?}", event);
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &State, data: &State, env: &Env) {
        debug!("REPAINTING");
        ctx.request_paint(); // just always request a paint
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &State, env: &Env) -> Size {
        debug!("Board::layout: {:?}", bc);

        let max_size = bc.max();
        let min_side = max_size.height.min(max_size.width);

        // save off the size of the square
        self.square_size = Size::new(min_side / 8.0f64, min_side / 8.0f64);

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
        let svg_scale = Affine::scale(self.square_size.height / 45.0f64);

        // go through and paint the board
        for row in 0..8 {
            for col in 0..8 {
                let point = Point {
                    x: self.square_size.width * row as f64,
                    y: self.square_size.height * col as f64,
                };

                let rect = Rect::from_origin_size(point, self.square_size);

                let env_clone = env.clone();

                // this paints the colored square
                ctx.paint_with_z_index(1, move |ctx| {
                    // TODO: make this constant... requires newer version of Rust :-|
                    let col_map:Vec<char> = vec!['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];

                    if (row + col) % 2 == 0 {
                        ctx.fill(rect, &WHITE);
                    } else {
                        ctx.fill(rect, &BROWN);
                    }

                    let mut label = TextLayout::<String>::from_text(format!("{}{}", col_map[row as usize], col+1));
                    label.set_text_color(Color::BLACK);

                    label.rebuild_if_needed(ctx.text(), &env_clone);
                    label.draw(ctx, point);
                });

                // create a board square from row & col
                let square = unsafe { Square::new(8 * row + col) };

                // first check to see if we're dragging a piece
                if let Some((dragging_square, pos)) = self.dragging_piece {
                    // ... and the current square is the one being dragged
                    if square == dragging_square {
                        let piece_svg = Self::square2svg(&data.game.current_position(), &dragging_square).unwrap();

                        // paint this piece in the middle of the mouse position
                        ctx.paint_with_z_index(2, move |ctx| {
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
                        let translate = Affine::translate((rect.min_y(), rect.min_x()) );

                        // debug!("RECT: {:?} -> ({}, {})", rect, rect.min_y(), rect.min_x());

                        // we have to do translate * svg_scale (not svg_scale * translate)
                        // otherwise, our translate is "in" the scale
                        piece_svg.to_piet(translate * svg_scale.clone(), ctx);
                    });
                }
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