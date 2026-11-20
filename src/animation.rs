use gltf::animation::{Interpolation, Property};
use macroquad::prelude::*;

#[derive(Clone, Debug)]
pub struct Channel {
    pub node: usize,
    pub property: Property,
    pub interpolation: Interpolation,
    pub times: Vec<f32>,
    pub values: Vec<f32>,
    pub width: usize,
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub name: String,
    pub duration: f32,
    pub channels: Vec<Channel>,
}

impl Channel {
    pub fn sample(&self, time: f32) -> Vec<f32> {
        let cubic = self.interpolation == Interpolation::CubicSpline;
        let stride = self.width * if cubic { 3 } else { 1 };
        let value = |key: usize| {
            let offset = key * stride + if cubic { self.width } else { 0 };
            &self.values[offset..offset + self.width]
        };
        let upper = self.times.partition_point(|t| *t <= time);
        let mut result = if upper == 0 {
            value(0).to_vec()
        } else if upper == self.times.len() {
            value(upper - 1).to_vec()
        } else {
            let a = upper - 1;
            let b = upper;
            let dt = self.times[b] - self.times[a];
            let t = (time - self.times[a]) / dt;
            let av = value(a);
            let bv = value(b);
            match self.interpolation {
                Interpolation::Step => av.to_vec(),
                Interpolation::Linear if self.property == Property::Rotation => {
                    let qa = Quat::from_xyzw(av[0], av[1], av[2], av[3]).normalize();
                    let qb = Quat::from_xyzw(bv[0], bv[1], bv[2], bv[3]).normalize();
                    qa.slerp(qb, t).to_array().to_vec()
                }
                Interpolation::Linear => av.iter().zip(bv).map(|(a, b)| a + (b - a) * t).collect(),
                Interpolation::CubicSpline => {
                    let t2 = t * t;
                    let t3 = t2 * t;
                    (0..self.width)
                        .map(|i| {
                            let out_tangent = self.values[a * stride + self.width * 2 + i];
                            let in_tangent = self.values[b * stride + i];
                            (2.0 * t3 - 3.0 * t2 + 1.0) * av[i]
                                + (t3 - 2.0 * t2 + t) * dt * out_tangent
                                + (-2.0 * t3 + 3.0 * t2) * bv[i]
                                + (t3 - t2) * dt * in_tangent
                        })
                        .collect()
                }
            }
        };
        if self.property == Property::Rotation {
            let q = Vec4::from_slice(&result).normalize_or_zero();
            result.copy_from_slice(&if q.length_squared() > 0.0 {
                q.to_array()
            } else {
                [0.0, 0.0, 0.0, 1.0]
            });
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn channel(interpolation: Interpolation) -> Channel {
        Channel {
            node: 0,
            property: Property::Translation,
            interpolation,
            times: vec![2.0, 4.0],
            values: vec![0.0, 4.0],
            width: 1,
        }
    }
    #[test]
    fn clamps_and_interpolates_nonzero_start() {
        let c = channel(Interpolation::Linear);
        assert_eq!(c.sample(0.0), vec![0.0]);
        assert_eq!(c.sample(3.0), vec![2.0]);
        assert_eq!(c.sample(8.0), vec![4.0]);
    }
    #[test]
    fn step_changes_at_key() {
        let c = channel(Interpolation::Step);
        assert_eq!(c.sample(3.99), vec![0.0]);
        assert_eq!(c.sample(4.0), vec![4.0]);
    }
    #[test]
    fn cubic_tangents_are_scaled_by_interval() {
        let mut c = channel(Interpolation::CubicSpline);
        c.values = vec![0.0, 0.0, 2.0, 0.0, 4.0, 0.0];
        assert!((c.sample(3.0)[0] - 2.5).abs() < 1e-6);
    }
    #[test]
    fn quaternion_uses_shortest_path_and_stays_normalized() {
        let mut c = channel(Interpolation::Linear);
        c.property = Property::Rotation;
        c.width = 4;
        c.values = vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0];
        let sample = c.sample(3.0);
        assert!((Vec4::from_slice(&sample).length() - 1.0).abs() < 1e-6);
        assert!(sample[3].abs() > 0.999);
    }
}
