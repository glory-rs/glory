use super::*;
#[cfg(feature = "paint")]
use glory_render::GraphicProps;
use glory_render::Props;

pub(super) fn check(
    node: glory_render::DescriptorView<'_>,
    patch: Option<&glory_render::Patch>,
) -> Result<(), Error> {
    if !matches!(node.props, Props::Graphic(_)) {
        return Ok(());
    }
    #[cfg(not(feature = "paint"))]
    {
        let _ = patch;
        return Err(Error::Unsupported("Apace vector graphics require paint"));
    }
    #[cfg(feature = "paint")]
    {
        let style = match patch {
            Some(glory_render::Patch::Style(glory_render::Change::Set(style))) => style.as_ref(),
            Some(glory_render::Patch::Style(glory_render::Change::Clear)) => return Ok(()),
            Some(_) => return Ok(()),
            None => node.style,
        };
        // The engine's widget callback supplies its border-box dimensions but
        // places the recording at the content origin. Until that callback has
        // a separate content size, reject these independent typed declarations.
        if style
            .padding
            .is_some_and(|edges| edges != glory_render::style::Edges::all(0.0))
        {
            return Err(Error::Unsupported("Apace graphic padding"));
        }
        if style
            .border_width
            .is_some_and(|edges| edges != glory_render::style::Edges::all(0.0))
        {
            return Err(Error::Unsupported("Apace graphic border widths"));
        }
        if style.object_fit.is_some() {
            return Err(Error::Unsupported("Apace graphic object fit"));
        }
        Ok(())
    }
}

#[cfg(feature = "paint")]
mod paint {
    use super::*;
    use anyrender::{PaintScene, Scene};
    use glory_render::{PathCommand, StrokeCap};
    use kurbo::{Affine, BezPath, Cap, Join, Rect, Stroke};
    use peniko::{Color, Fill};

    struct Graphic(
        GraphicProps,
        Option<Rc<graphic_motion::Animation>>,
        Option<Rc<graphic_motion::Transition>>,
    );
    impl apace_dom::Widget for Graphic {
        fn requires_redraw(&self) -> bool {
            self.1.as_ref().is_some_and(|state| state.running.get())
                || self.2.as_ref().is_some_and(|state| state.running.get())
        }
        fn intrinsic_sizes(&self) -> apace_dom::IntrinsicSizes {
            apace_dom::IntrinsicSizes {
                width: Some(self.0.width as f32),
                height: Some(self.0.height as f32),
                ratio: Some((self.0.width / self.0.height) as f32),
            }
        }
        fn paint(
            &mut self,
            _context: &mut dyn anyrender::RenderContext,
            _styles: &apace_dom::node::ComputedStyles,
            width: u32,
            height: u32,
            _scale: f64,
        ) -> Scene {
            let mut scene = Scene::new();
            if width == 0 || height == 0 {
                return scene;
            }
            let sx = f64::from(width) / self.0.width;
            let sy = f64::from(height) / self.0.height;
            let scale = sx.min(sy);
            let mut transform = Affine::new([
                scale,
                0.0,
                0.0,
                scale,
                (f64::from(width) - self.0.width * scale) / 2.0,
                (f64::from(height) - self.0.height * scale) / 2.0,
            ]);
            if let (Some(animation), Some(state)) = (self.0.animation, &self.1)
                && state.running.get()
            {
                let angle = animation
                    .radians(state.clock.get())
                    .expect("validated host clock and animation");
                let center = (self.0.width / 2., self.0.height / 2.);
                transform = transform
                    * Affine::translate(center)
                    * Affine::rotate(angle)
                    * Affine::translate((-center.0, -center.1));
            }
            let mut path = BezPath::new();
            for (index, target) in self.0.commands.iter().enumerate() {
                let command = self
                    .2
                    .as_ref()
                    .map_or(*target, |state| state.command(index, *target));
                match &command {
                    PathCommand::Move(point) => path.move_to((point.x, point.y)),
                    PathCommand::Line(point) => path.line_to((point.x, point.y)),
                    PathCommand::Cubic { first, second, end } => {
                        path.curve_to((first.x, first.y), (second.x, second.y), (end.x, end.y))
                    }
                    PathCommand::Close => path.close_path(),
                }
            }
            scene.push_clip_layer(
                Fill::NonZero,
                Affine::IDENTITY,
                &Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
            );
            if let Some(color) = self.0.fill {
                scene.fill(
                    Fill::NonZero,
                    transform,
                    Color::from_rgba8(color.0, color.1, color.2, color.3),
                    None,
                    &path,
                );
            }
            if let Some(stroke) = self.0.stroke {
                let cap = match stroke.cap {
                    StrokeCap::Butt => Cap::Butt,
                    StrokeCap::Round => Cap::Round,
                    StrokeCap::Square => Cap::Square,
                };
                let style = Stroke::new(stroke.width)
                    .with_caps(cap)
                    .with_join(Join::Round);
                let color = stroke.color;
                scene.stroke(
                    &style,
                    transform,
                    Color::from_rgba8(color.0, color.1, color.2, color.3),
                    None,
                    &path,
                );
            }
            scene.pop_layer();
            scene
        }
    }
    impl Dom {
        pub(crate) fn install_graphic(
            &mut self,
            id: NodeId,
            graphic: &GraphicProps,
            previous: Option<&[glory_render::PathCommand]>,
        ) -> Result<(), Error> {
            let main = self.range(id)?.part(Part::Main)?;
            let animation = self.motion.install(id, graphic.animation);
            let transition =
                self.motion
                    .install_transition(id, graphic.transition, &graphic.commands, previous);
            self.doc.set_custom_widget(
                main,
                Box::new(Graphic(graphic.clone(), animation, transition)),
            );
            self.invalidate();
            Ok(())
        }
    }
}
