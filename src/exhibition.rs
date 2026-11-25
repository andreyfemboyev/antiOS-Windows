use crate::{
    composition::{self, Dialog, Interface},
    configuration::{Args, Preferences, Quality},
    export,
    renderer::{Layer, Renderer},
    simulation::{World, DEFAULT_SEED},
    typography::Typography,
};
use anyhow::Result;
use std::path::Path;

pub fn export(args: Args) -> Result<()> {
    let seed = args.seed.unwrap_or(DEFAULT_SEED);
    let mut world = World::new(seed);
    let size = args.size.unwrap_or((1920, 1200));
    let preferences = Preferences {
        quality: args.quality.unwrap_or(Quality::High),
        reduced_motion: args.reduced_motion,
        ..Default::default()
    };
    let typography = Typography::new();
    let mut renderer = pollster::block_on(Renderer::new(
        None,
        size,
        args.backend,
        preferences.quality,
        &typography,
    ))?;
    eprintln!(
        "antiOS / {} ({:?}) / world {}",
        renderer.adapter.name, renderer.adapter.backend, seed
    );
    if let Some(directory) = &args.frames {
        export::empty_directory(directory)?;
    }
    let count = if args.render.is_some() {
        1
    } else {
        args.count.unwrap_or(args.fps * 10)
    };
    let start = u64::from(args.at_frame);
    let mut art = Layer::default();
    let mut chrome = Layer::default();
    for index in start.saturating_sub(15)..start + u64::from(count) {
        let target = index * 60 / u64::from(args.fps);
        while world.frame < target {
            world.step();
        }
        composition::artwork(
            &mut art,
            &typography,
            &world,
            renderer.quality,
            preferences.reduced_motion,
        );
        if !args.clean {
            composition::chrome(
                &mut chrome,
                &typography,
                &world,
                &Interface {
                    dialog: Dialog::None,
                    paused: false,
                    muted: true,
                    fullscreen: false,
                    preferences: &preferences,
                    status: None,
                },
            );
        }
        renderer.draw(
            &art,
            &chrome,
            composition::frame(&world, preferences.reduced_motion),
            false,
        )?;
        if index < start {
            continue;
        }
        let numbered;
        let path: &Path = if let Some(path) = &args.render {
            path
        } else {
            numbered = args
                .frames
                .as_ref()
                .expect("sequence requested")
                .join(format!("{:08}.png", index - start));
            &numbered
        };
        renderer.save(path)?;
        if count == 1 {
            eprintln!("antiOS / {}", path.display());
        }
    }
    if let (Some(directory), Some(out)) = (&args.frames, &args.mp4) {
        export::encode(directory, out, args.fps)?;
        eprintln!("antiOS / {}", out.display());
    }
    Ok(())
}
