use glb_viewer::scene::Bounds;
use macroquad::prelude::*;

const ZOOM_LOG_STEP: f32 = 0.04;
const ZOOM_RESPONSE: f32 = 12.0;

// Miniquad 0.4 forwards native scroll units: Windows WHEEL_DELTA (120),
// macOS point deltas (10 for a non-precise wheel), X11 button steps (1).
const WHEEL_UNITS_PER_STEP: f32 = if cfg!(target_os = "windows") {
    120.0
} else if cfg!(target_os = "macos") {
    10.0
} else {
    1.0
};

pub struct OrbitCamera {
    pub focus: Vec3,
    pub distance: f32,
    target_distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub radius: f32,
    drag: Option<MouseButton>,
    previous: Vec2,
}
impl OrbitCamera {
    pub fn new(bounds: Bounds) -> Self {
        let mut camera = Self {
            focus: Vec3::ZERO,
            distance: 4.0,
            target_distance: 4.0,
            yaw: 0.42,
            pitch: 0.18,
            radius: 1.0,
            drag: None,
            previous: Vec2::ZERO,
        };
        camera.fit(bounds, 1.0);
        camera
    }
    pub fn fit(&mut self, bounds: Bounds, aspect: f32) {
        self.focus = bounds.center();
        self.radius = bounds.radius();
        let half_angle = (22.5_f32.to_radians().tan() * aspect.min(1.0)).atan();
        self.distance = self.radius / half_angle.sin() * 1.14;
        self.target_distance = self.distance;
    }
    pub fn position(&self) -> Vec3 {
        self.focus
            + self.distance
                * vec3(
                    self.pitch.cos() * self.yaw.sin(),
                    self.pitch.sin(),
                    self.pitch.cos() * self.yaw.cos(),
                )
    }
    pub fn update(&mut self, viewport: Rect, blocked: bool, subscriber: usize) {
        if blocked {
            self.drag = None;
        }
        // Replay every pointer transition, including a complete drag between frames.
        // This matters when CPU skinning a dense model slows the frame loop.
        let mut input = PointerInput {
            camera: self,
            viewport,
            blocked,
            dpi: miniquad::window::dpi_scale(),
        };
        macroquad::input::utils::repeat_all_miniquad_input(&mut input, subscriber);
        if self
            .drag
            .is_some_and(|button| !is_mouse_button_down(button))
        {
            self.drag = None;
        }
        self.previous = Vec2::from(mouse_position());
        self.smooth_zoom(get_frame_time());
    }
    fn scroll_zoom(&mut self, delta: f32) {
        if !delta.is_finite() {
            return;
        }
        // Preserve fractional touchpad input, but limit unusually large events.
        let steps = (delta / WHEEL_UNITS_PER_STEP).clamp(-4.0, 4.0);
        self.target_distance = (self.target_distance * (-steps * ZOOM_LOG_STEP).exp())
            .clamp(self.radius * 0.025, self.radius * 200.0);
    }
    fn smooth_zoom(&mut self, dt: f32) {
        // Time-based easing keeps the same response at high and low frame rates.
        let blend = 1.0 - (-ZOOM_RESPONSE * dt.max(0.0)).exp();
        self.distance += (self.target_distance - self.distance) * blend;
        if (self.target_distance - self.distance).abs() <= self.target_distance * 0.00001 {
            self.distance = self.target_distance;
        }
    }
    fn pan(&mut self, delta: Vec2, height: f32) {
        let forward = (self.focus - self.position()).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();
        let scale = 2.0 * self.distance * 22.5_f32.to_radians().tan() / height.max(1.0);
        self.focus += (-right * delta.x + up * delta.y) * scale;
    }
    pub fn camera(&self, viewport: Rect) -> Camera3D {
        let dpi = miniquad::window::dpi_scale();
        Camera3D {
            position: self.position(),
            target: self.focus,
            up: Vec3::Y,
            fovy: 45.0_f32.to_radians(),
            aspect: Some(viewport.w / viewport.h.max(1.0)),
            viewport: Some((
                (viewport.x * dpi) as i32,
                ((screen_height() - viewport.y - viewport.h) * dpi) as i32,
                (viewport.w * dpi) as i32,
                (viewport.h * dpi) as i32,
            )),
            z_near: (self.distance * 0.001).max(self.radius * 0.0001),
            z_far: self.distance + self.radius * 220.0,
            ..Default::default()
        }
    }
}

struct PointerInput<'a> {
    camera: &'a mut OrbitCamera,
    viewport: Rect,
    blocked: bool,
    dpi: f32,
}
impl miniquad::EventHandler for PointerInput<'_> {
    fn update(&mut self) {}
    fn draw(&mut self) {}
    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
        let point = vec2(x, y) / self.dpi;
        self.camera.previous = point;
        if !self.blocked
            && self.viewport.contains(point)
            && matches!(button, MouseButton::Left | MouseButton::Right)
        {
            self.camera.drag = Some(button);
        }
    }
    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        let point = vec2(x, y) / self.dpi;
        let delta = point - self.camera.previous;
        if !self.blocked {
            match self.camera.drag {
                Some(MouseButton::Left) => {
                    self.camera.yaw -= delta.x * 0.007;
                    self.camera.pitch = (self.camera.pitch + delta.y * 0.007).clamp(-1.52, 1.52);
                }
                Some(MouseButton::Right) => self.camera.pan(delta, self.viewport.h),
                _ => {}
            }
        }
        self.camera.previous = point;
    }
    fn mouse_button_up_event(&mut self, button: MouseButton, x: f32, y: f32) {
        self.mouse_motion_event(x, y);
        if self.camera.drag == Some(button) {
            self.camera.drag = None;
        }
    }
    fn mouse_wheel_event(&mut self, _x: f32, y: f32) {
        if !self.blocked && self.viewport.contains(self.camera.previous) {
            self.camera.scroll_zoom(y);
        }
    }
    fn window_minimized_event(&mut self) {
        self.camera.drag = None;
    }
}

pub fn project(point: Vec3, matrix: Mat4, viewport: Rect) -> Option<Vec2> {
    let p = matrix * point.extend(1.0);
    if p.w <= 0.0 {
        return None;
    }
    let ndc = p.truncate() / p.w;
    if ndc.z < -1.0 || ndc.z > 1.0 {
        return None;
    }
    Some(vec2(
        viewport.x + (ndc.x + 1.0) * 0.5 * viewport.w,
        viewport.y + (1.0 - ndc.y) * 0.5 * viewport.h,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use miniquad::EventHandler;
    fn zoom_camera() -> OrbitCamera {
        OrbitCamera::new(Bounds {
            min: Vec3::splat(-1.0),
            max: Vec3::ONE,
        })
    }
    #[test]
    fn wheel_notch_zooms_gently_without_an_instant_jump() {
        let mut camera = zoom_camera();
        let before = camera.distance;
        let mut input = PointerInput {
            camera: &mut camera,
            viewport: Rect::new(0.0, 0.0, 800.0, 600.0),
            blocked: false,
            dpi: 1.0,
        };
        input.mouse_motion_event(400.0, 300.0);
        // Regression: the native Windows delta is 120, not one.
        let delta = if cfg!(target_os = "windows") {
            120.0
        } else if cfg!(target_os = "macos") {
            10.0
        } else {
            1.0
        };
        input.mouse_wheel_event(0.0, delta);
        assert_eq!(camera.distance, before);
        assert!((0.95..0.97).contains(&(camera.target_distance / before)));
        camera.smooth_zoom(1.0 / 60.0);
        assert!(camera.distance < before && camera.distance > camera.target_distance);
        assert!(camera.distance > before * 0.99);
    }
    #[test]
    fn zoom_smoothing_is_frame_rate_independent_and_does_not_overshoot() {
        let mut slow = zoom_camera();
        let mut fast = zoom_camera();
        slow.scroll_zoom(WHEEL_UNITS_PER_STEP * 3.0);
        fast.scroll_zoom(WHEEL_UNITS_PER_STEP * 3.0);
        for _ in 0..15 {
            slow.smooth_zoom(1.0 / 30.0);
        }
        for _ in 0..60 {
            fast.smooth_zoom(1.0 / 120.0);
        }
        assert!((slow.distance - fast.distance).abs() < slow.distance * 0.00001);
        assert!(slow.distance >= slow.target_distance);
        assert!(slow.distance - slow.target_distance < slow.target_distance * 0.001);
    }
    #[test]
    fn fit_cancels_pending_zoom_and_large_wheel_events_are_bounded() {
        let mut camera = zoom_camera();
        let before = camera.distance;
        camera.scroll_zoom(WHEEL_UNITS_PER_STEP * 1000.0);
        assert!(camera.target_distance > before * 0.85);
        camera.fit(
            Bounds {
                min: Vec3::ZERO,
                max: Vec3::splat(5.0),
            },
            1.0,
        );
        let fitted = camera.distance;
        camera.smooth_zoom(1.0);
        assert_eq!(camera.distance, fitted);
        camera.scroll_zoom(-WHEEL_UNITS_PER_STEP);
        camera.smooth_zoom(1.0 / 60.0);
        assert!(camera.distance > fitted && camera.distance < camera.target_distance);
    }
    #[test]
    fn complete_left_and_right_drags_between_frames_are_not_lost() {
        let mut camera = OrbitCamera::new(Bounds {
            min: Vec3::splat(-1.0),
            max: Vec3::ONE,
        });
        let yaw = camera.yaw;
        let focus = camera.focus;
        let mut input = PointerInput {
            camera: &mut camera,
            viewport: Rect::new(0.0, 0.0, 800.0, 600.0),
            blocked: false,
            dpi: 2.0,
        };
        input.mouse_button_down_event(MouseButton::Left, 400.0, 400.0);
        input.mouse_motion_event(600.0, 400.0);
        input.mouse_button_up_event(MouseButton::Left, 600.0, 400.0);
        assert!((input.camera.yaw - (yaw - 0.7)).abs() < 1e-5);
        assert_eq!(input.camera.focus, focus);
        let yaw = input.camera.yaw;
        input.mouse_button_down_event(MouseButton::Right, 400.0, 400.0);
        input.mouse_button_up_event(MouseButton::Right, 600.0, 460.0);
        assert_eq!(input.camera.yaw, yaw);
        assert!(input.camera.focus.distance(focus) > 0.01);
        assert!(input.camera.drag.is_none());
    }
    #[test]
    fn pan_moves_eye_and_focus_together_in_view_plane() {
        let mut camera = OrbitCamera::new(Bounds {
            min: Vec3::splat(-1.0),
            max: Vec3::ONE,
        });
        let before_eye = camera.position();
        let before_focus = camera.focus;
        let view = (before_focus - before_eye).normalize();
        camera.pan(vec2(100.0, -30.0), 600.0);
        let shift = camera.focus - before_focus;
        assert!(shift.length() > 0.01);
        assert!(shift.dot(view).abs() < 1e-5);
        assert!(((camera.position() - before_eye) - shift).length() < 1e-5);
    }
    #[test]
    fn fit_accounts_for_portrait_viewport() {
        let bounds = Bounds {
            min: Vec3::splat(-1.0),
            max: Vec3::ONE,
        };
        let mut camera = OrbitCamera::new(bounds);
        let wide = camera.distance;
        camera.fit(bounds, 0.5);
        assert!(camera.distance > wide);
        assert_eq!(camera.focus, bounds.center());
    }
}
