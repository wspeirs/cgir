use druid::{Widget, EventCtx, LifeCycle, PaintCtx, LifeCycleCtx, BoxConstraints, Size, LayoutCtx, Event, Env, UpdateCtx, Point, Rect, Color, Affine};
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
        Size {
            width: min_side,
            height: min_side,
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &State, env: &Env) {
        debug!("Board::paint");

        let size: Size = ctx.size();
        let w0 = size.width / 8.0f64;
        let h0 = size.height / 8.0f64;

        let square_size = Size {
            width: w0,
            height: h0,
        };

        // self.cell_size = square_size;

        // go through and paint the board
        for row in 0..8 {
            for col in 0..8 {
                let point = Point {
                    x: w0 * row as f64,
                    y: h0 * col as f64,
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

        let white_pawn = include_str!("./assets/svg/white_pawn.svg").parse::<SvgData>().unwrap();

        // we want our pieces on top of our squares
        ctx.paint_with_z_index(2, move |ctx| {
            // let offset_matrix = FillStrat::default().affine_to_fill(ctx.size(), white_pawn.size());

            let clip_rect = Rect::ZERO.with_size(ctx.size());

            // The SvgData's to_piet function does not clip to the svg's size
            // CairoRenderContext is very like druids but with some extra goodies like clip
            ctx.clip(clip_rect);
            white_pawn.to_piet(Affine::default(), ctx);
        });
    }

    fn type_name(&self) -> &'static str {
        "board"
    }
}