use druid::{Widget, EventCtx, LifeCycle, PaintCtx, LifeCycleCtx, BoxConstraints, Size, LayoutCtx, Event, Env, UpdateCtx, Point, Rect, Color, Affine, WidgetExt};
use druid::RenderContext;
use druid::widget::{Svg, SvgData};
use crate::State;

use log::debug;

const BROWN :Color = Color::rgb8(0x91, 0x67, 0x2c);
const WHITE :Color = Color::WHITE;


pub struct Board {

}

impl Widget<State> for Board {
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

        // go through and paint the board
        for row in 0..8 {
            for col in 0..8 {
                let point = Point {
                    x: square_width * row as f64,
                    y: square_height * col as f64,
                };

                let rect = Rect::from_origin_size(point, square_size);

                ctx.paint_with_z_index(1, move |ctx| {
                    if (row + col) % 2 == 0 {
                        ctx.fill(rect, &WHITE);
                    } else {
                        ctx.fill(rect, &BROWN);
                    }
                });
            }
        }

        let white_pawn_svg_data = include_str!("./assets/svg/white_pawn.svg").parse::<SvgData>().unwrap();

        let data_clone = data.clone();
        let env_clone = env.clone();

        // we want our pieces on top of our squares
        ctx.paint_with_z_index(2, move |ctx| {
            // compute the scale ratio between the space size and the piece
            let affine_matrix = Affine::scale(square_width / 45.0f64);
            white_pawn_svg_data.to_piet(affine_matrix, ctx);
        });
    }

    fn type_name(&self) -> &'static str {
        "board"
    }
}