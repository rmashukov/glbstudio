#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod camera;
mod player;
mod render;
mod ui;

use camera::OrbitCamera;
use glb_viewer::scene::Scene;
use macroquad::{camera::Camera, prelude::*};
use player::Player;
use render::{GpuScene, Renderer, ViewMode};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread,
};
use ui::{Settings, Ui};

fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: miniquad::conf::Conf {
            window_title: "GLB Studio".into(),
            window_width: 1280,
            window_height: 820,
            window_resizable: true,
            high_dpi: true,
            sample_count: 4,
            platform: miniquad::conf::Platform {
                apple_gfx_api: miniquad::conf::AppleGfxApi::Metal,
                swap_interval: Some(1),
                ..Default::default()
            },
            ..Default::default()
        },
        update_on: None,
        draw_call_vertex_capacity: 65_536,
        draw_call_index_capacity: 262_144,
        ..Default::default()
    }
}
fn start_load(path: Option<PathBuf>) -> Receiver<Result<Scene, String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = std::panic::catch_unwind(|| match path {
            Some(path) => Scene::from_path(&path),
            None => Scene::demo(),
        });
        let result = match result {
            Ok(result) => result.map_err(|e| format!("{e:#}")),
            Err(_) => Err("The file contains malformed glTF data. Please export it again.".into()),
        };
        let _ = sender.send(result);
    });
    receiver
}

#[macroquad::main(window_conf)]
async fn main() {
    let renderer = match Renderer::new() {
        Ok(renderer) => renderer,
        Err(error) => {
            if std::env::var_os("GLB_VIEWER_SMOKE_FRAMES").is_some() {
                eprintln!("Graphics initialization failed: {error:#}");
                std::process::exit(1);
            }
            rfd::MessageDialog::new()
                .set_title("GLB Studio - graphics error")
                .set_description(format!(
                    "Could not initialize the graphics shaders: {error}"
                ))
                .set_level(rfd::MessageLevel::Error)
                .show();
            return;
        }
    };
    let mut scene = Scene::demo().expect("The built-in demo is validated by cargo test");
    let mut gpu = GpuScene::new(&scene);
    let mut camera = OrbitCamera::new(scene.bounds);
    let camera_input = macroquad::input::utils::register_input_subscriber();
    let mut player = Player::new(&scene);
    let mut settings = Settings::default();
    let mut ui = Ui::new();
    let mut error: Option<String> = None;
    let mut loader = std::env::args_os()
        .nth(1)
        .map(|arg| start_load(Some(PathBuf::from(arg))));
    let mut fps = 60.0;
    let mut frames = 0_u32;
    let smoke_frames = std::env::var("GLB_VIEWER_SMOKE_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u32>().ok());
    camera.fit(scene.bounds, Ui::viewport().w / Ui::viewport().h);
    loop {
        if let Some(receiver) = &loader {
            match receiver.try_recv() {
                Ok(result) => {
                    loader = None;
                    match result {
                        Ok(new_scene) => {
                            let new_gpu = GpuScene::new(&new_scene);
                            player = Player::new(&new_scene);
                            camera.fit(new_scene.bounds, Ui::viewport().w / Ui::viewport().h);
                            if new_scene.joint_nodes.is_empty() {
                                settings.skeleton = false;
                            }
                            for note in &new_scene.warnings {
                                eprintln!("Import note: {note}");
                            }
                            scene = new_scene;
                            gpu = new_gpu;
                            error = None;
                        }
                        Err(message) => error = Some(message),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    loader = None;
                    error = Some("The model loader stopped unexpectedly.".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let viewport = Ui::viewport();
        let blocked = error.is_some() || loader.is_some();
        let mut open = false;
        let mut typed = Vec::new();
        while let Some(c) = get_char_pressed() {
            typed.push(c.to_ascii_lowercase());
        }
        let shortcut =
            |key, letter| is_key_pressed(key) || (!is_key_down(key) && typed.contains(&letter));
        if !blocked {
            open = shortcut(KeyCode::O, 'o');
            if shortcut(KeyCode::Key1, '1') {
                settings.mode = ViewMode::Shaded;
            }
            if shortcut(KeyCode::Key2, '2') {
                settings.mode = ViewMode::Wireframe;
            }
            if shortcut(KeyCode::Key3, '3') {
                settings.mode = ViewMode::Overlay;
            }
            if shortcut(KeyCode::S, 's') && !scene.joint_nodes.is_empty() {
                settings.skeleton = !settings.skeleton;
            }
            if shortcut(KeyCode::G, 'g') {
                settings.grid = !settings.grid;
            }
            if shortcut(KeyCode::Space, ' ') {
                player.toggle(&scene);
            }
            if is_key_pressed(KeyCode::Left) {
                player.select(-1, &scene);
            }
            if is_key_pressed(KeyCode::Right) {
                player.select(1, &scene);
            }
            if shortcut(KeyCode::F, 'f') || is_key_pressed(KeyCode::Home) {
                if is_key_pressed(KeyCode::Home) {
                    camera.yaw = 0.42;
                    camera.pitch = 0.18;
                }
                camera.fit(scene.posed_bounds(), viewport.w / viewport.h);
            }
        }
        for dropped in get_dropped_files() {
            if loader.is_none()
                && let Some(path) = dropped.path
            {
                loader = Some(start_load(Some(path)));
                error = None;
            }
        }
        camera.update(viewport, blocked, camera_input);
        player.update(get_frame_time(), &scene);
        if player.dirty {
            scene.pose(player.clip, player.time);
            gpu.update(&scene);
            player.dirty = false;
        }
        clear_background(Color::from_hex(0x17222e));
        let view = camera.camera(viewport);
        set_camera(&view);
        if settings.grid {
            render::grid(scene.bounds);
        }
        renderer.draw(&scene, &gpu, &view, settings.mode, settings.exposure);
        set_default_camera();
        if settings.skeleton {
            render::skeleton(&scene, view.matrix(), viewport);
        }
        fps += ((1.0 / get_frame_time().max(0.001)).min(999.0) - fps) * 0.04;
        let actions = ui.draw(
            &scene,
            &mut settings,
            &mut player,
            loader.is_some(),
            error.as_deref(),
            fps,
        );
        if actions.dismiss_error {
            error = None;
        }
        if actions.fit {
            camera.fit(scene.posed_bounds(), viewport.w / viewport.h);
        }
        if actions.demo {
            loader = Some(start_load(None));
        }
        if (open || actions.open) && loader.is_none() {
            // AppKit requires native modal dialogs on the main thread.
            // Decoding and scene construction then run on a worker thread.
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Open a glTF model")
                .add_filter("glTF models", &["glb", "gltf"])
                .pick_file()
            {
                loader = Some(start_load(Some(path)));
            }
        }
        if loader.is_none() {
            frames += 1;
        }
        if let Some(limit) = smoke_frames {
            if let Some(error) = &error {
                eprintln!("Smoke test failed: {error}");
                std::process::exit(1);
            }
            if frames == limit / 3 {
                settings.mode = ViewMode::Wireframe;
            }
            if frames == limit * 2 / 3 {
                settings.mode = ViewMode::Overlay;
                settings.skeleton = true;
            }
            if frames >= limit && loader.is_none() {
                println!(
                    "SMOKE OK: {} / {} triangles / {} frames / {:.0} FPS",
                    scene.name,
                    scene.triangles(),
                    frames,
                    fps
                );
                break;
            }
        }
        next_frame().await;
    }
}
