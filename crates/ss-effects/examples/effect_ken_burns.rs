//! Ken Burns effect example.
//!
//! Generates a self-contained demo project with synthetic test images and a
//! multi-segment Ken Burns pan/zoom effect with crossfade transitions.
//!
//! ```sh
//! cargo run --example effect_ken_burns -p ss-effects
//! ```

use std::path::Path;

use image::{Rgba, RgbaImage};
use ss_effects::{KenBurnsBuilder, KenBurnsDirection, KenBurnsParams, KenBurnsSegmentParams};
use ss_project_builder::{ProjectBuilder, ProjectParams};

/// Output directory for generated assets and project JSON.
const OUT_DIR: &str = "examples/effects/ken_burns";

/// Test images to generate (color name, bright checker, dark checker).
const COLORS: &[(&str, Rgba<u8>, Rgba<u8>)] = &[
    ("red", Rgba([220, 50, 50, 255]), Rgba([120, 20, 20, 255])),
    ("green", Rgba([50, 180, 50, 255]), Rgba([20, 100, 20, 255])),
    ("blue", Rgba([50, 80, 220, 255]), Rgba([20, 30, 120, 255])),
    (
        "yellow",
        Rgba([230, 210, 40, 255]),
        Rgba([130, 120, 15, 255]),
    ),
    (
        "magenta",
        Rgba([200, 50, 200, 255]),
        Rgba([110, 20, 110, 255]),
    ),
];

/// Checkerboard tile size in pixels.
const TILE: u32 = 40;

fn main() {
    let out = Path::new(OUT_DIR);
    std::fs::create_dir_all(out).expect("failed to create output directory");

    generate_images(out);
    let project_path = build_project(out);

    println!("\n✓ Generated Ken Burns demo project:");
    println!("  {}/", out.display());
    println!("  ├── red.png");
    println!("  ├── green.png");
    println!("  ├── blue.png");
    println!("  ├── yellow.png");
    println!("  ├── magenta.png");
    println!("  └── project.json");
    println!();
    println!("Preview in editor:");
    println!(
        "  cargo run --release -p ss-editor -- {}",
        project_path.display()
    );
    println!();
    println!("Render to MP4:");
    println!(
        "  cargo run --release -p ss-render -- {}",
        project_path.display()
    );
}

/// Generates checkerboard 400×400 PNG images.
fn generate_images(dir: &Path) {
    for (name, bright, dark) in COLORS {
        let mut img = RgbaImage::new(400, 400);
        for y in 0..400 {
            for x in 0..400 {
                let checker = ((x / TILE) + (y / TILE)).is_multiple_of(2);
                img.put_pixel(x, y, if checker { *bright } else { *dark });
            }
        }
        let path = dir.join(format!("{name}.png"));
        img.save(&path).unwrap_or_else(|e| {
            panic!("failed to save {}: {e}", path.display());
        });
        eprintln!("  wrote {} (400×400 {} checkerboard)", path.display(), name);
    }
}

/// Builds the Ken Burns project and writes project.json.
fn build_project(dir: &Path) -> std::path::PathBuf {
    let hold = 2.0;
    let fade = 1.0;
    let total_duration = COLORS.len() as f64 * (hold + fade);

    let kb_params = KenBurnsParams::builder()
        .duration(total_duration)
        .resolution([400, 400])
        .default_hold_duration(hold)
        .default_fade_duration(fade)
        .default_pan_fraction(0.9)
        .default_zoom_end(3.7)
        .build();

    let directions = [
        KenBurnsDirection::ZoomInLeft,
        KenBurnsDirection::ZoomOutCenter,
        KenBurnsDirection::ZoomInRight,
        KenBurnsDirection::ZoomInUp,
        KenBurnsDirection::FocalPoint {
            start: [0.3, 0.3],
            end: [0.7, 0.7],
        },
    ];

    let mut builder = KenBurnsBuilder::new(kb_params);
    for (i, (name, _bright, _dark)) in COLORS.iter().enumerate() {
        let segment = KenBurnsSegmentParams::builder()
            .image_path(format!("{name}.png"))
            .direction(directions[i].clone())
            .build();
        builder = builder.add_segment(segment);
    }

    let clips = builder.build();
    eprintln!("  built {} Ken Burns clips", clips.len());

    let project_params = ProjectParams::builder()
        .resolution([400, 400])
        .fps(30)
        .duration(total_duration)
        .output(format!("{OUT_DIR}/output.mp4"))
        .background([0, 0, 0, 255])
        .build();

    let project_path = dir.join("project.json");
    ProjectBuilder::new(project_params)
        .add_clips_at(0.0, clips)
        .to_json_file(&project_path)
        .unwrap_or_else(|e| {
            panic!("failed to write project.json: {e:?}");
        });
    eprintln!("  wrote {}", project_path.display());

    project_path
}
