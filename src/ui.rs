use crate::{player::Player, render::ViewMode};
use glb_viewer::scene::Scene;
use macroquad::prelude::*;
use std::sync::OnceLock;

static FONT: OnceLock<Font> = OnceLock::new();

pub const TEXT: Color = Color::new(0.89, 0.93, 0.96, 1.0);
pub const MUTED: Color = Color::new(0.49, 0.57, 0.65, 1.0);
pub const ACCENT: Color = Color::new(0.39, 0.90, 0.75, 1.0);
const PANEL: Color = Color::new(0.075, 0.105, 0.14, 1.0);
const BORDER: Color = Color::new(0.15, 0.20, 0.25, 1.0);
const CONTROL: Color = Color::new(0.12, 0.16, 0.21, 1.0);

pub struct Settings {
    pub mode: ViewMode,
    pub skeleton: bool,
    pub grid: bool,
    pub exposure: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: ViewMode::Shaded,
            skeleton: false,
            grid: true,
            exposure: 1.0,
        }
    }
}
#[derive(Default)]
pub struct Actions {
    pub open: bool,
    pub demo: bool,
    pub fit: bool,
    pub dismiss_error: bool,
}
pub struct Ui {
    active_slider: Option<u32>,
    pub blocked: bool,
}
impl Ui {
    pub fn new() -> Self {
        FONT.get_or_init(|| {
            load_ttf_font_from_bytes(include_bytes!("../assets/NotoSans-Regular.ttf"))
                .expect("Bundled Noto Sans is valid")
        });
        Self {
            active_slider: None,
            blocked: false,
        }
    }
    pub fn viewport() -> Rect {
        let panel = if screen_width() >= 860.0 { 286.0 } else { 0.0 };
        Rect::new(
            0.0,
            64.0,
            (screen_width() - panel).max(1.0),
            (screen_height() - 194.0).max(1.0),
        )
    }
    fn button(&self, r: Rect, label: &str, selected: bool, enabled: bool) -> bool {
        let hover = enabled && !self.blocked && r.contains(Vec2::from(mouse_position()));
        let bg = if selected {
            Color::new(0.15, 0.33, 0.31, 1.0)
        } else if hover {
            Color::new(0.19, 0.25, 0.30, 1.0)
        } else {
            CONTROL
        };
        rounded(r, 6.0, bg);
        let color = if !enabled {
            MUTED.with_alpha(0.4)
        } else if selected {
            ACCENT
        } else {
            TEXT
        };
        let dim = measure_text(label, FONT.get(), 16, 1.0);
        text(
            label,
            r.x + (r.w - dim.width) * 0.5,
            r.y + r.h * 0.5 + 5.0,
            16.0,
            color,
        );
        hover && is_mouse_button_pressed(MouseButton::Left)
    }
    fn toggle(&self, x: f32, y: f32, w: f32, label: &str, value: &mut bool, enabled: bool) {
        let r = Rect::new(x, y, w, 32.0);
        text(label, x, y + 21.0, 16.0, if enabled { TEXT } else { MUTED });
        let switch = Rect::new(x + w - 34.0, y + 8.0, 34.0, 18.0);
        rounded(
            switch,
            9.0,
            if *value && enabled {
                Color::new(0.18, 0.53, 0.44, 1.0)
            } else {
                CONTROL
            },
        );
        draw_circle(
            switch.x + if *value { 25.0 } else { 9.0 },
            switch.y + 9.0,
            6.0,
            if enabled { TEXT } else { MUTED },
        );
        if enabled
            && !self.blocked
            && r.contains(Vec2::from(mouse_position()))
            && is_mouse_button_pressed(MouseButton::Left)
        {
            *value = !*value;
        }
    }
    fn slider(
        &mut self,
        id: u32,
        r: Rect,
        value: &mut f32,
        min: f32,
        max: f32,
        enabled: bool,
    ) -> bool {
        let mouse = Vec2::from(mouse_position());
        if enabled
            && !self.blocked
            && r.contains(mouse)
            && is_mouse_button_pressed(MouseButton::Left)
        {
            self.active_slider = Some(id);
        }
        let active = enabled
            && !self.blocked
            && self.active_slider == Some(id)
            && (is_mouse_button_down(MouseButton::Left)
                || is_mouse_button_pressed(MouseButton::Left));
        if active {
            *value = min + ((mouse.x - r.x) / r.w).clamp(0.0, 1.0) * (max - min);
        }
        let ratio = if max > min {
            ((*value - min) / (max - min)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        rounded(
            Rect::new(r.x, r.y + r.h * 0.5 - 2.0, r.w, 4.0),
            2.0,
            CONTROL,
        );
        if enabled {
            rounded(
                Rect::new(r.x, r.y + r.h * 0.5 - 2.0, (r.w * ratio).max(4.0), 4.0),
                2.0,
                ACCENT,
            );
            draw_circle(
                r.x + r.w * ratio,
                r.y + r.h * 0.5,
                if active { 6.0 } else { 4.5 },
                TEXT,
            );
        }
        active
    }
    pub fn draw(
        &mut self,
        scene: &Scene,
        settings: &mut Settings,
        player: &mut Player,
        loading: bool,
        error: Option<&str>,
        fps: f32,
    ) -> Actions {
        self.blocked = error.is_some();
        if !is_mouse_button_down(MouseButton::Left) {
            self.active_slider = None;
        }
        let mut actions = Actions::default();
        let width = screen_width();
        let height = screen_height();
        let viewport = Self::viewport();
        draw_rectangle(0.0, 0.0, width, 64.0, Color::from_hex(0x101821));
        draw_line(0.0, 63.0, width, 63.0, 1.0, BORDER);
        cube_icon(vec2(29.0, 31.0), 11.0, ACCENT);
        text("GLB STUDIO", 53.0, 30.0, 21.0, TEXT);
        text("MODEL INSPECTOR", 54.0, 48.0, 10.0, MUTED);
        if width > 720.0 {
            draw_line(222.0, 19.0, 222.0, 45.0, 1.0, BORDER);
            text_fit(
                &scene.name,
                242.0,
                38.0,
                (width - 570.0).max(50.0),
                16.0,
                MUTED,
            );
        }
        if self.button(
            Rect::new(width - 252.0, 17.0, 92.0, 32.0),
            "Demo",
            false,
            !loading,
        ) {
            actions.demo = true;
        }
        if self.button(
            Rect::new(width - 148.0, 17.0, 128.0, 32.0),
            "Open GLB",
            true,
            !loading,
        ) {
            actions.open = true;
        }
        rounded(
            Rect::new(22.0, 84.0, 110.0, 27.0),
            5.0,
            PANEL.with_alpha(0.92),
        );
        draw_circle(34.0, 97.0, 3.0, ACCENT);
        text("Perspective", 45.0, 102.0, 14.0, TEXT);
        if self.button(
            Rect::new(viewport.w - 108.0, 82.0, 86.0, 29.0),
            "Fit  [F]",
            false,
            true,
        ) {
            actions.fit = true;
        }
        if settings.skeleton && !scene.joint_nodes.is_empty() {
            text("SKELETON / X-RAY", 23.0, 136.0, 11.0, ACCENT);
        }
        if width >= 860.0 {
            self.inspector(scene, settings, viewport.w, height);
        }
        self.timeline(scene, player, viewport.w, height - 130.0);
        draw_rectangle(0.0, height - 28.0, width, 28.0, Color::from_hex(0x101821));
        draw_line(0.0, height - 28.0, width, height - 28.0, 1.0, BORDER);
        draw_circle(
            18.0,
            height - 13.0,
            3.0,
            if loading { GOLD } else { ACCENT },
        );
        let status = if loading {
            "Loading model...".into()
        } else if scene.warnings.is_empty() {
            format!(
                "{} triangles   /   {} joints   /   {} animations",
                number(scene.triangles()),
                scene.joint_nodes.len(),
                scene.clips.len()
            )
        } else {
            format!(
                "{} triangles   /   {} import notes",
                number(scene.triangles()),
                scene.warnings.len()
            )
        };
        text_fit(&status, 30.0, height - 9.0, width - 160.0, 12.0, MUTED);
        text(
            &format!("{fps:.0} FPS"),
            width - 78.0,
            height - 9.0,
            12.0,
            MUTED,
        );
        if loading {
            let r = Rect::new(
                viewport.w * 0.5 - 110.0,
                viewport.y + viewport.h * 0.5 - 27.0,
                220.0,
                54.0,
            );
            rounded(r, 9.0, PANEL.with_alpha(0.96));
            text("Loading your model...", r.x + 25.0, r.y + 33.0, 18.0, TEXT);
        }
        if let Some(error) = error {
            draw_rectangle(0.0, 0.0, width, height, BLACK.with_alpha(0.62));
            let w = (width - 48.0).min(580.0);
            let r = Rect::new((width - w) * 0.5, (height - 270.0) * 0.5, w, 270.0);
            rounded(r, 12.0, PANEL);
            text(
                "Could not open this model",
                r.x + 24.0,
                r.y + 40.0,
                23.0,
                TEXT,
            );
            text_wrapped(
                error,
                vec2(r.x + 24.0, r.y + 76.0),
                w - 48.0,
                16.0,
                MUTED,
                6,
            );
            self.blocked = false;
            actions.dismiss_error = self.button(
                Rect::new(r.x + w - 134.0, r.y + 214.0, 110.0, 34.0),
                "Dismiss",
                true,
                true,
            ) || is_key_pressed(KeyCode::Escape);
            self.blocked = true;
        }
        actions
    }
    fn inspector(&mut self, scene: &Scene, settings: &mut Settings, left: f32, height: f32) {
        draw_rectangle(left, 64.0, 286.0, height - 92.0, PANEL);
        draw_line(left, 64.0, left, height - 28.0, 1.0, BORDER);
        let x = left + 22.0;
        let w = 242.0;
        text("SCENE", x, 96.0, 11.0, ACCENT);
        text_fit(&scene.name, x, 126.0, w, 21.0, TEXT);
        text(
            &format!(
                "{} meshes  /  {} textures",
                scene.primitives.len(),
                scene.textures.len()
            ),
            x,
            149.0,
            13.0,
            MUTED,
        );
        stat(x, 179.0, "VERTICES", &number(scene.vertices()));
        stat(x + 126.0, 179.0, "TRIANGLES", &number(scene.triangles()));
        separator(x, 232.0, w);
        text("DISPLAY", x, 259.0, 11.0, ACCENT);
        for (index, (label, mode)) in [
            ("Shaded", ViewMode::Shaded),
            ("Wire", ViewMode::Wireframe),
            ("Overlay", ViewMode::Overlay),
        ]
        .into_iter()
        .enumerate()
        {
            if self.button(
                Rect::new(x + index as f32 * 82.0, 276.0, 77.0, 34.0),
                label,
                settings.mode == mode,
                true,
            ) {
                settings.mode = mode;
            }
        }
        self.toggle(
            x,
            324.0,
            w,
            "Skeleton x-ray",
            &mut settings.skeleton,
            !scene.joint_nodes.is_empty(),
        );
        self.toggle(x, 361.0, w, "Ground grid", &mut settings.grid, true);
        if scene.joint_nodes.is_empty() {
            text("No skeleton in this model", x, 417.0, 12.0, MUTED);
        } else {
            text(
                &format!(
                    "{} joints  /  {} bones",
                    scene.joint_nodes.len(),
                    scene.bones.len()
                ),
                x,
                417.0,
                12.0,
                MUTED,
            );
        }
        separator(x, 437.0, w);
        text("STUDIO LIGHTING", x, 465.0, 11.0, ACCENT);
        text("Exposure", x, 492.0, 15.0, TEXT);
        text(
            &format!("{:.2}x", settings.exposure),
            x + 197.0,
            492.0,
            14.0,
            MUTED,
        );
        self.slider(
            1,
            Rect::new(x, 501.0, w, 27.0),
            &mut settings.exposure,
            0.35,
            2.5,
            true,
        );
        text("Soft key, cool fill, rim light", x, 550.0, 12.0, MUTED);
        if height > 770.0 {
            separator(x, 574.0, w);
            text("NAVIGATION", x, 601.0, 11.0, ACCENT);
            for (i, (key, action)) in [
                ("LMB", "Orbit around focus"),
                ("RMB", "Pan focus"),
                ("SCROLL", "Zoom"),
                ("F / HOME", "Fit / reset view"),
            ]
            .iter()
            .enumerate()
            {
                text(key, x, 627.0 + i as f32 * 24.0, 11.0, MUTED);
                text(action, x + 80.0, 627.0 + i as f32 * 24.0, 13.0, TEXT);
            }
        }
        if !scene.warnings.is_empty() && height > 880.0 {
            text_wrapped(&scene.warnings.join(". "), vec2(x, 752.0), w, 12.0, GOLD, 4);
        }
    }
    fn timeline(&mut self, scene: &Scene, player: &mut Player, width: f32, top: f32) {
        draw_rectangle(0.0, top, width, 102.0, PANEL);
        draw_line(0.0, top, width, top, 1.0, BORDER);
        let enabled = !scene.clips.is_empty();
        if self.button(
            Rect::new(22.0, top + 16.0, 68.0, 30.0),
            if player.playing { "Pause" } else { "Play" },
            player.playing,
            enabled,
        ) {
            player.toggle(scene);
        }
        if self.button(
            Rect::new(100.0, top + 16.0, 30.0, 30.0),
            "<",
            false,
            enabled,
        ) {
            player.select(-1, scene);
        }
        let name = player.clip.map_or(
            if enabled {
                "Rest pose"
            } else {
                "No animations"
            },
            |i| scene.clips[i].name.as_str(),
        );
        let label_width = (width - 408.0).max(80.0);
        text_fit(
            name,
            141.0,
            top + 36.0,
            label_width,
            15.0,
            if enabled { TEXT } else { MUTED },
        );
        if self.button(
            Rect::new(150.0 + label_width, top + 16.0, 30.0, 30.0),
            ">",
            false,
            enabled,
        ) {
            player.select(1, scene);
        }
        if width > 580.0 {
            if self.button(
                Rect::new(width - 210.0, top + 16.0, 66.0, 30.0),
                &format!("{}x", player.speed),
                false,
                enabled,
            ) {
                player.speed = if player.speed < 0.5 {
                    0.5
                } else if player.speed < 1.0 {
                    1.0
                } else if player.speed < 1.5 {
                    1.5
                } else if player.speed < 2.0 {
                    2.0
                } else {
                    0.25
                };
            }
            if self.button(
                Rect::new(width - 134.0, top + 16.0, 58.0, 30.0),
                "Loop",
                player.looping,
                enabled,
            ) {
                player.looping = !player.looping;
            }
            if self.button(
                Rect::new(width - 66.0, top + 16.0, 44.0, 30.0),
                "|<",
                false,
                enabled,
            ) {
                player.time = 0.0;
                player.dirty = true;
            }
        }
        let duration = player.duration(scene);
        if self.slider(
            2,
            Rect::new(24.0, top + 54.0, (width - 164.0).max(30.0), 24.0),
            &mut player.time,
            0.0,
            duration,
            enabled && duration > 0.0,
        ) {
            player.playing = false;
            player.dirty = true;
        }
        text(
            &format!("{:05.2} / {:05.2}", player.time, duration),
            width - 125.0,
            top + 71.0,
            13.0,
            MUTED,
        );
        text(
            "SPACE  Play / pause     1 2 3  Display     S  Skeleton     G  Grid",
            24.0,
            top + 92.0,
            11.0,
            MUTED,
        );
    }
}

pub fn text(value: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text_ex(
        value,
        x,
        y,
        TextParams {
            font: FONT.get(),
            font_size: size as u16,
            color,
            ..Default::default()
        },
    );
}
fn text_fit(value: &str, x: f32, y: f32, width: f32, size: f32, color: Color) {
    let mut s = value.to_string();
    if measure_text(&s, FONT.get(), size as u16, 1.0).width > width {
        while !s.is_empty()
            && measure_text(format!("{s}..."), FONT.get(), size as u16, 1.0).width > width
        {
            s.pop();
        }
        s.push_str("...");
    }
    text(&s, x, y, size, color);
}
fn text_wrapped(value: &str, origin: Vec2, width: f32, size: f32, color: Color, limit: usize) {
    let mut line = String::new();
    let mut row = 0;
    for word in value.split_whitespace() {
        if measure_text(format!("{line} {word}"), FONT.get(), size as u16, 1.0).width > width
            && !line.is_empty()
        {
            text_fit(
                &line,
                origin.x,
                origin.y + row as f32 * (size + 6.0),
                width,
                size,
                color,
            );
            row += 1;
            line.clear();
            if row >= limit {
                return;
            }
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if row < limit {
        text_fit(
            &line,
            origin.x,
            origin.y + row as f32 * (size + 6.0),
            width,
            size,
            color,
        );
    }
}
fn stat(x: f32, y: f32, label: &str, value: &str) {
    text(label, x, y, 10.0, MUTED);
    text(value, x, y + 27.0, 24.0, TEXT);
}
fn separator(x: f32, y: f32, w: f32) {
    draw_line(x, y, x + w, y, 1.0, BORDER);
}
fn rounded(r: Rect, radius: f32, color: Color) {
    let radius = radius.min(r.w * 0.5).min(r.h * 0.5);
    draw_rectangle(r.x + radius, r.y, r.w - radius * 2.0, r.h, color);
    draw_rectangle(r.x, r.y + radius, r.w, r.h - radius * 2.0, color);
    for (x, y) in [
        (r.x + radius, r.y + radius),
        (r.x + r.w - radius, r.y + radius),
        (r.x + radius, r.y + r.h - radius),
        (r.x + r.w - radius, r.y + r.h - radius),
    ] {
        draw_circle(x, y, radius, color);
    }
}
fn cube_icon(center: Vec2, r: f32, color: Color) {
    let points = [
        vec2(0.0, -r),
        vec2(r, -r * 0.5),
        vec2(r, r * 0.5),
        vec2(0.0, r),
        vec2(-r, r * 0.5),
        vec2(-r, -r * 0.5),
    ];
    for i in 0..6 {
        let a = center + points[i];
        let b = center + points[(i + 1) % 6];
        draw_line(a.x, a.y, b.x, b.y, 1.8, color);
    }
    for i in [1, 3, 5] {
        let b = center + points[i];
        draw_line(center.x, center.y, b.x, b.y, 1.8, color);
    }
}
fn number(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    out
}
