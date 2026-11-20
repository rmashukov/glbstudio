use glb_viewer::scene::Scene;
use std::path::PathBuf;

fn main() {
    let mut failed = false;
    for path in std::env::args_os().skip(1).map(PathBuf::from) {
        match Scene::from_path(&path) {
            Ok(mut scene) => {
                println!(
                    "{}: {} vertices, {} triangles, {} textures, {} joints, {} clips",
                    path.display(),
                    scene.vertices(),
                    scene.triangles(),
                    scene.textures.len(),
                    scene.joint_nodes.len(),
                    scene.clips.len()
                );
                for index in 0..scene.clips.len() {
                    for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
                        scene.pose(Some(index), scene.clips[index].duration * fraction);
                        if !scene.primitives.iter().all(|p| {
                            p.posed_positions
                                .iter()
                                .chain(&p.posed_normals)
                                .all(|v| v.is_finite())
                        }) {
                            failed = true;
                            eprintln!("Non-finite pose in clip {index}");
                        }
                    }
                }
                for warning in &scene.warnings {
                    println!("  Note: {warning}");
                }
            }
            Err(error) => {
                eprintln!("{}: {error:#}", path.display());
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
