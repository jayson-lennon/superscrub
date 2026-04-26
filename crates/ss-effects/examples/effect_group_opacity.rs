//! Group opacity example.
//!
//! Demonstrates non-destructive opacity via groups. Four colored squares are
//! placed in the four quadrants of a 400×400 canvas, then grouped together.
//! The group receives a single opacity animation that fades the entire group
//! from fully visible to transparent — without modifying any child clip values.
//!
//! ```sh
//! cargo run --example effect_group_opacity -p ss-effects
//! ```

use std::path::Path;

use image::{Rgba, RgbaImage};
use ss_effects::group_opacity;
use ss_project_builder::{
    AnimBuilder, ClipBuilder, ClipParams, ProjectBuilder, ProjectParams,
    clip_builder::sizing,
};

/// Output directory for generated assets and project JSON.
const OUT_DIR: &str = "examples/effects/group_opacity";

/// Quadrant definitions: (name, fill color, x, y).
///
/// Each square is 200×200 placed to fill one quadrant of the 400×400 canvas.
const QUADRANTS: &[(&str, Rgba<u8>, i32, i32)] = &[
    ("top_left", Rgba([220, 60, 60, 255]), 0, 0),
    ("top_right", Rgba([60, 180, 60, 255]), 200, 0),
    ("bottom_left", Rgba([60, 80, 220, 255]), 0, 200),
    ("bottom_right", Rgba([220, 200, 40, 255]), 200, 200),
];

fn main() {
    let out = Path::new(OUT_DIR);
    std::fs::create_dir_all(out).expect("failed to create output directory");

    generate_images(out);
    let project_path = build_project(out);

    println!("\n✓ Generated group opacity demo project:");
    println!("  {}/", out.display());
    println!("  ├── top_left.png");
    println!("  ├── top_right.png");
    println!("  ├── bottom_left.png");
    println!("  ├── bottom_right.png");
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

/// Generates solid-color 200×200 PNG images for each quadrant.
fn generate_images(dir: &Path) {
    for (name, color, _x, _y) in QUADRANTS {
        let mut img = RgbaImage::from_pixel(200, 200, *color);
        // Draw a thin 2px border so the squares are visually distinct even when overlapping.
        let border = Rgba([255, 255, 255, 80]);
        for i in 0..200 {
            for b in 0..2 {
                img.put_pixel(i, b, border);
                img.put_pixel(i, 199 - b, border);
                img.put_pixel(b, i, border);
                img.put_pixel(199 - b, i, border);
            }
        }
        let path = dir.join(format!("{name}.png"));
        img.save(&path)
            .unwrap_or_else(|e| panic!("failed to save {}: {e}", path.display()));
        println!("  wrote {} (200×200 {})", path.display(), name);
    }
}

/// Builds the group opacity project and writes project.json.
fn build_project(dir: &Path) -> std::path::PathBuf {
    let total_duration = 6.0;

    // Build one clip per quadrant, each sized to fit its 200×200 rect via Cover mode.
    let clips: Vec<ClipBuilder> = QUADRANTS
        .iter()
        .map(|(name, _color, x, y)| {
            ClipBuilder::new(
                ClipParams::builder()
                    .id(name.to_string())
                    .path(format!("{name}.png"))
                    .end_time(total_duration)
                    .sizing(sizing::fit_rect_cover(*x, *y, 200, 200))
                    .build(),
            )
        })
        .collect();

    println!("  built {} quadrant clips", clips.len());

    // Wrap all clips in a group with opacity animating from 1.0 → 0.0.
    // This is non-destructive: each child clip retains its own values.
    // The group's opacity multiplies into children during interpolation.
    let group = group_opacity("quadrants", clips, 1.0)
        .expect("group_opacity should succeed with valid clips");

    // Replace the single constant keyframe with a fade animation.
    // We rebuild the group with an explicit opacity track: visible for 2s, then fade out over 3s.
    let group_with_fade = {
        let mut g = group;
        g.animations = vec![AnimBuilder::opacity()
            .keyframe(0.0, 1.0)
            .keyframe(2.0, 1.0)
            .keyframe(5.0, 0.0)
            .build()];
        g
    };

    println!("  wrapped in group \"quadrants\" with opacity fade 1.0 → 0.0");

    let project_params = ProjectParams::builder()
        .resolution([400, 400])
        .fps(30)
        .duration(total_duration)
        .output(format!("{OUT_DIR}/output.mp4"))
        .background([30, 30, 34, 255])
        .build();

    let project_path = dir.join("project.json");
    ProjectBuilder::new(project_params)
        .add_item(group_with_fade)
        .to_json_file(&project_path)
        .unwrap_or_else(|e| {
            panic!("failed to write project.json: {e:?}");
        });
    println!("  wrote {}", project_path.display());

    project_path
}
