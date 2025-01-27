use geom::ArrowCap;
use map_gui::tools::ColorNetwork;
use map_model::{IntersectionID, RoadID, NORMAL_LANE_THICKNESS};
use widgetry::mapspace::{ObjectID, ToggleZoomed, World};
use widgetry::{
    Color, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State,
    Text, TextExt, Toggle, VerticalAlignment, Widget,
};

use super::rat_runs::{find_rat_runs, RatRuns};
use super::Neighborhood;
use crate::app::{App, Transition};

pub struct BrowseRatRuns {
    panel: Panel,
    rat_runs: RatRuns,
    current_idx: usize,

    draw_path: ToggleZoomed,
    draw_heatmap: ToggleZoomed,
    world: World<Obj>,
    neighborhood: Neighborhood,
}

impl BrowseRatRuns {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        neighborhood: Neighborhood,
    ) -> Box<dyn State<App>> {
        let rat_runs = ctx.loading_screen("find rat runs", |_, timer| {
            find_rat_runs(
                &app.primary.map,
                &neighborhood,
                &app.session.modal_filters,
                timer,
            )
        });
        let mut colorer = ColorNetwork::no_fading(app);
        colorer.ranked_roads(rat_runs.count_per_road.clone(), &app.cs.good_to_bad_red);
        // TODO These two will be on different scales, which'll look really weird!
        colorer.ranked_intersections(
            rat_runs.count_per_intersection.clone(),
            &app.cs.good_to_bad_red,
        );
        let world = make_world(ctx, app, &neighborhood, &rat_runs);

        let mut state = BrowseRatRuns {
            panel: Panel::empty(ctx),
            rat_runs,
            current_idx: 0,
            draw_path: ToggleZoomed::empty(ctx),
            draw_heatmap: colorer.build(ctx),
            neighborhood,
            world,
        };
        state.recalculate(ctx, app);
        Box::new(state)
    }

    fn recalculate(&mut self, ctx: &mut EventCtx, app: &App) {
        if self.rat_runs.paths.is_empty() {
            self.panel = Panel::new_builder(Widget::col(vec![
                ctx.style()
                    .btn_outline
                    .text("Back to editing modal filters")
                    .hotkey(Key::Escape)
                    .build_def(ctx),
                "No rat runs detected".text_widget(ctx),
            ]))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
            .build(ctx);
            return;
        }

        self.panel = Panel::new_builder(Widget::col(vec![
            ctx.style()
                .btn_outline
                .text("Back to editing modal filters")
                .hotkey(Key::Escape)
                .build_def(ctx),
            Line("Warning: preliminary results")
                .fg(Color::RED)
                .into_widget(ctx),
            Widget::row(vec![
                "Rat runs:".text_widget(ctx).centered_vert(),
                ctx.style()
                    .btn_prev()
                    .disabled(self.current_idx == 0)
                    .hotkey(Key::LeftArrow)
                    .build_widget(ctx, "previous rat run"),
                Text::from(
                    Line(format!(
                        "{}/{}",
                        self.current_idx + 1,
                        self.rat_runs.paths.len()
                    ))
                    .secondary(),
                )
                .into_widget(ctx)
                .centered_vert(),
                ctx.style()
                    .btn_next()
                    .disabled(self.current_idx == self.rat_runs.paths.len() - 1)
                    .hotkey(Key::RightArrow)
                    .build_widget(ctx, "next rat run"),
            ]),
            // TODO This should disable the individual path controls, or maybe even be a different
            // state entirely...
            Toggle::checkbox(
                ctx,
                "show heatmap of all rat-runs",
                Key::R,
                self.panel
                    .maybe_is_checked("show heatmap of all rat-runs")
                    .unwrap_or(true),
            ),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);

        let mut draw_path = ToggleZoomed::builder();
        let color = Color::RED;
        let path = &self.rat_runs.paths[self.current_idx];
        if let Some(pl) = path.trace(&app.primary.map) {
            // TODO This produces a really buggy shape sometimes!
            let shape = pl.make_arrow(3.0 * NORMAL_LANE_THICKNESS, ArrowCap::Triangle);
            draw_path.unzoomed.push(color.alpha(0.8), shape.clone());
            draw_path.zoomed.push(color.alpha(0.5), shape);

            draw_path
                .unzoomed
                .append(map_gui::tools::start_marker(ctx, pl.first_pt(), 2.0));
            draw_path
                .zoomed
                .append(map_gui::tools::start_marker(ctx, pl.first_pt(), 0.5));

            draw_path
                .unzoomed
                .append(map_gui::tools::goal_marker(ctx, pl.last_pt(), 2.0));
            draw_path
                .zoomed
                .append(map_gui::tools::goal_marker(ctx, pl.last_pt(), 0.5));
        }
        self.draw_path = draw_path.build(ctx);
    }
}

impl State<App> for BrowseRatRuns {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "Back to editing modal filters" => {
                    return Transition::ConsumeState(Box::new(|state, ctx, app| {
                        let state = state.downcast::<BrowseRatRuns>().ok().unwrap();
                        vec![super::viewer::Viewer::new_state(
                            ctx,
                            app,
                            state.neighborhood,
                        )]
                    }));
                }
                "previous rat run" => {
                    self.current_idx -= 1;
                    self.panel
                        .set_checked("show heatmap of all rat-runs", false);
                    self.recalculate(ctx, app);
                }
                "next rat run" => {
                    self.current_idx += 1;
                    self.panel
                        .set_checked("show heatmap of all rat-runs", false);
                    self.recalculate(ctx, app);
                }
                _ => unreachable!(),
            }
        }

        // Just trigger tooltips; no other interactions possible
        let _ = self.world.event(ctx);

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);

        if self.panel.is_checked("show heatmap of all rat-runs") {
            self.draw_heatmap.draw(g);
            self.world.draw(g);
        } else {
            self.draw_path.draw(g);
        }

        g.redraw(&self.neighborhood.fade_irrelevant);
        self.neighborhood.draw_filters.draw(g);
        if g.canvas.is_unzoomed() {
            self.neighborhood.labels.draw(g, app);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Obj {
    InteriorRoad(RoadID),
    InteriorIntersection(IntersectionID),
}
impl ObjectID for Obj {}

fn make_world(
    ctx: &mut EventCtx,
    app: &App,
    neighborhood: &Neighborhood,
    rat_runs: &RatRuns,
) -> World<Obj> {
    let map = &app.primary.map;
    let mut world = World::bounded(map.get_bounds());

    for r in &neighborhood.orig_perimeter.interior {
        world
            .add(Obj::InteriorRoad(*r))
            .hitbox(map.get_r(*r).get_thick_polygon())
            .drawn_in_master_batch()
            // TODO Not sure if tooltip() without this should imply it?
            .draw_hovered(GeomBatch::new())
            .tooltip(Text::from(format!(
                "{} rat-runs cross this street",
                rat_runs.count_per_road.get(*r)
            )))
            .build(ctx);
    }
    for i in &neighborhood.interior_intersections {
        world
            .add(Obj::InteriorIntersection(*i))
            .hitbox(map.get_i(*i).polygon.clone())
            .drawn_in_master_batch()
            .draw_hovered(GeomBatch::new())
            .tooltip(Text::from(format!(
                "{} rat-runs cross this intersection",
                rat_runs.count_per_intersection.get(*i)
            )))
            .build(ctx);
    }

    world.initialize_hover(ctx);

    world
}
