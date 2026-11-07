//! Which player the card is about: the one under the crosshair, or the nearest one in view.
//! Line of sight is the caller's to check, in the order given here.

use crate::config::Targeting;

/// A player that could be targeted, at this frame's interpolated position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub id: u32,
    /// Feet.
    pub position: [f64; 3],
    /// Half the box's width.
    pub radius: f64,
    pub height: f64,
}

impl Candidate {
    pub fn center(&self) -> [f64; 3] {
        let p = self.position;
        [p[0], p[1] + self.height * 0.5, p[2]]
    }
    /// Points line of sight is tested to: the head, then the middle.
    pub fn sight_points(&self) -> [[f64; 3]; 2] {
        let p = self.position;
        [[p[0], p[1] + self.height * 0.85, p[2]], self.center()]
    }
}

/// The look direction for a yaw and pitch in degrees (yaw 0 looks south, +z).
pub fn forward(yaw: f32, pitch: f32) -> [f64; 3] {
    let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    [
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    ]
}

/// The candidates that qualify, best first.
///
/// Under the crosshair: the look ray goes through the player's box, padded a little and more
/// with distance so a far player isn't a pixel hunt; the nearest along the ray comes first.
/// Nearest: any player within a cone as wide as the view is tall, nearest first.
pub fn ranked(
    eye: [f64; 3],
    look: [f64; 3],
    candidates: &[Candidate],
    range: f64,
    mode: Targeting,
    fov_degrees: f32,
) -> Vec<u32> {
    let mut scored: Vec<(f64, u32)> = candidates
        .iter()
        .filter_map(|c| {
            let to = sub(c.center(), eye);
            let along = dot(to, look);
            let distance = length(to);
            if along <= 0.0 || distance > range {
                return None;
            }
            match mode {
                Targeting::Crosshair => {
                    let pad = 0.15 + distance * 0.02;
                    let p = c.position;
                    let min = [p[0] - c.radius - pad, p[1] - pad, p[2] - c.radius - pad];
                    let max = [
                        p[0] + c.radius + pad,
                        p[1] + c.height + pad,
                        p[2] + c.radius + pad,
                    ];
                    ray_box(eye, look, min, max).map(|t| (t, c.id))
                }
                Targeting::Nearest => {
                    let half_cone = f64::from(fov_degrees.clamp(30.0, 170.0)).to_radians() * 0.5;
                    let angle = (along / distance.max(1e-6)).clamp(-1.0, 1.0).acos();
                    (angle <= half_cone).then_some((distance, c.id))
                }
            }
        })
        .collect();
    scored.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, id)| id).collect()
}

/// Where a ray enters a box (0 when it starts inside), by the slab method.
fn ray_box(origin: [f64; 3], dir: [f64; 3], min: [f64; 3], max: [f64; 3]) -> Option<f64> {
    let (mut near, mut far) = (0.0f64, f64::INFINITY);
    for axis in 0..3 {
        if dir[axis].abs() < 1e-12 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return None;
            }
            continue;
        }
        let a = (min[axis] - origin[axis]) / dir[axis];
        let b = (max[axis] - origin[axis]) / dir[axis];
        near = near.max(a.min(b));
        far = far.min(a.max(b));
        if near > far {
            return None;
        }
    }
    Some(near)
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(id: u32, x: f64, z: f64) -> Candidate {
        Candidate {
            id,
            position: [x, 64.0, z],
            radius: 0.3,
            height: 1.8,
        }
    }

    const EYE: [f64; 3] = [0.0, 65.62, 0.0];

    #[test]
    fn the_crosshair_picks_the_nearest_player_on_the_ray() {
        let look = forward(0.0, 0.0); // south
        let players = [
            player(1, 0.2, 20.0),
            player(2, -0.1, 8.0),
            player(3, 4.0, 10.0),
        ];
        let ids = ranked(EYE, look, &players, 48.0, Targeting::Crosshair, 70.0);
        assert_eq!(ids, [2, 1]);
        // Behind, or too far: nobody.
        let ids = ranked(
            EYE,
            forward(180.0, 0.0),
            &players,
            48.0,
            Targeting::Crosshair,
            70.0,
        );
        assert!(ids.is_empty());
        let ids = ranked(EYE, look, &players, 5.0, Targeting::Crosshair, 70.0);
        assert!(ids.is_empty());
    }

    #[test]
    fn far_players_get_a_little_more_room() {
        let look = forward(0.0, 0.0);
        // A box edge 0.6 blocks off the ray: missed at 10 blocks, caught at 40.
        let off = |z| {
            ranked(
                EYE,
                look,
                &[player(1, 0.9, z)],
                48.0,
                Targeting::Crosshair,
                70.0,
            )
        };
        assert!(off(10.0).is_empty());
        assert_eq!(off(40.0), [1]);
    }

    #[test]
    fn nearest_takes_anyone_in_view() {
        let look = forward(0.0, 0.0);
        let players = [
            player(1, 6.0, 10.0),
            player(2, 0.0, 30.0),
            player(3, -20.0, 1.0),
        ];
        assert_eq!(
            ranked(EYE, look, &players, 48.0, Targeting::Nearest, 70.0),
            [1, 2]
        );
    }

    #[test]
    fn west_is_where_yaw_90_looks() {
        let f = forward(90.0, 0.0);
        assert!((f[0] + 1.0).abs() < 1e-9 && f[2].abs() < 1e-9);
        let down = forward(0.0, 90.0);
        assert!((down[1] + 1.0).abs() < 1e-9);
    }
}
