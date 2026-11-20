use glb_viewer::scene::Scene;

pub struct Player {
    pub clip: Option<usize>,
    pub time: f32,
    pub playing: bool,
    pub speed: f32,
    pub looping: bool,
    pub dirty: bool,
}
impl Player {
    pub fn new(scene: &Scene) -> Self {
        Self {
            clip: (!scene.clips.is_empty()).then_some(0),
            time: 0.0,
            playing: !scene.clips.is_empty(),
            speed: 1.0,
            looping: true,
            dirty: true,
        }
    }
    pub fn duration(&self, scene: &Scene) -> f32 {
        self.clip.map_or(0.0, |i| scene.clips[i].duration)
    }
    pub fn select(&mut self, direction: i32, scene: &Scene) {
        if scene.clips.is_empty() {
            return;
        }
        let slot = (self.clip.map_or(0, |i| i + 1) as i32 + direction)
            .rem_euclid(scene.clips.len() as i32 + 1);
        self.clip = (slot > 0).then(|| slot as usize - 1);
        self.time = 0.0;
        self.playing = self.clip.is_some();
        self.dirty = true;
    }
    pub fn toggle(&mut self, scene: &Scene) {
        if scene.clips.is_empty() {
            return;
        }
        if self.clip.is_none() {
            self.clip = Some(0);
            self.dirty = true;
        }
        if self.time >= self.duration(scene) {
            self.time = 0.0;
            self.dirty = true;
        }
        self.playing = !self.playing;
    }
    pub fn update(&mut self, dt: f32, scene: &Scene) {
        if !self.playing {
            return;
        }
        let duration = self.duration(scene);
        if duration <= 0.0 {
            self.playing = false;
            return;
        }
        self.time += dt.min(0.1) * self.speed;
        if self.time >= duration {
            if self.looping {
                self.time %= duration;
            } else {
                self.time = duration;
                self.playing = false;
            }
        }
        self.dirty = true;
    }
}
