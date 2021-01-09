use druid::{Widget, EventCtx, LifeCycle, PaintCtx, LifeCycleCtx, BoxConstraints, Size, LayoutCtx, Event, Env, UpdateCtx, Point, Rect, Color, Affine, WidgetExt};
use druid::RenderContext;
use druid::widget::{Svg, SvgData};
use crate::State;
use std::fs::File;
use std::io::prelude::*;


use log::debug;
use chess::{Square, Piece};

const BROWN :Color = Color::rgb8(0x91, 0x67, 0x2c);
const WHITE :Color = Color::WHITE;


pub struct BoardWidget { }

impl Widget<State> for BoardWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut State, env: &Env) {
        // debug!("Board::event: {:?}", event);
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &State, env: &Env) {
        debug!("Board::lifecycle: {:?}", event);
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &State, data: &State, env: &Env) {
        unimplemented!()
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &State, env: &Env) -> Size {
        debug!("Board::layout: {:?}", bc);

        let max_size = bc.max();
        let min_side = max_size.height.min(max_size.width);

        // return something that's square
        Size {
            width: min_side,
            height: min_side,
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &State, env: &Env) {
        debug!("Board::paint");

        let size: Size = ctx.size();
        let square_width = size.width / 8.0f64;
        let square_height = size.height / 8.0f64;

        let square_size = Size {
            width: square_width,
            height: square_height,
        };

        debug!("CTX SIZE: {:?} SQUARE SIZE: {:?}", size, square_size);

        // compute the scale ratio between the space size and the piece
        let svg_scale = Affine::scale(square_width / 45.0f64);

        // go through and paint the board
        for row in 0..8 {
            for col in 0..8 {
                let point = Point {
                    x: square_width * row as f64,
                    y: square_height * col as f64,
                };

                let rect = Rect::from_origin_size(point, square_size);

                // this paints the colored square
                ctx.paint_with_z_index(1, move |ctx| {
                    if (row + col) % 2 == 0 {
                        ctx.fill(rect, &WHITE);
                    } else {
                        ctx.fill(rect, &BROWN);
                    }
                });

                // figure out if we have a piece on the board here
                // and paint that piece if so
                let square = unsafe { Square::new(8 * row + col) };

                if let Some(piece) = data.board.piece_on(square.clone()) {
                    debug!("{:?} => {:?}", square, piece);

                    let color = data.board.color_on(square).unwrap();
                    let mut file = File::open(format!("/home/wspeirs/src/cgir/src/assets/svg/{}.svg", piece.to_string(color))).unwrap();
                    let mut contents = String::new();
                    file.read_to_string(&mut contents).unwrap();

                    let svg_data = contents.parse::<SvgData>().unwrap();

                    let data_clone = data.clone();
                    let env_clone = env.clone();

                    // we want our pieces on top of our squares
                    ctx.paint_with_z_index(2, move |ctx| {
                        let translate = Affine::translate((rect.min_y() * (45.0f64 / square_width), rect.min_x() * (45.0f64 / square_width)) );

                        debug!("RECT: {:?} -> ({}, {})", rect, rect.min_y(), rect.min_x());

                        svg_data.to_piet(svg_scale.clone() * translate, ctx);
                    });
                }
            }
        }
    }

    fn type_name(&self) -> &'static str {
        "board"
    }
}