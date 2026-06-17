//! Catmull-Rom spline with arc-length parameterisation.
//!
//! The track is defined as a polyline of lat/lon points; the spline interpolates
//! a smooth curve through them and supports lookup by distance travelled.

use bevy::math::Vec3;

/// Centripetal cubic Hermite interpolation between control points, with a
/// precomputed table mapping arc length to parameter t.
pub struct Spline {
    points: Vec<Vec3>,
    tension: f32,
    /// (t, cumulative_length) samples in ascending t order.
    arc: Vec<(f32, f32)>,
    length: f32,
}

impl Spline {
    pub fn new(points: Vec<Vec3>, tension: f32, samples: usize) -> Self {
        let mut s = Self {
            points,
            tension,
            arc: Vec::with_capacity(samples + 1),
            length: 0.0,
        };
        s.rebuild_arc(samples);
        s
    }

    pub fn length(&self) -> f32 {
        self.length
    }

    pub fn position(&self, t: f32) -> Vec3 {
        let (i, u) = self.segment(t);
        let (p0, p1, p2, p3) = self.tangent_neighbours(i);
        cubic(p0, p1, p2, p3, u, self.tension)
    }

    pub fn tangent(&self, t: f32) -> Vec3 {
        let (i, u) = self.segment(t);
        let (p0, p1, p2, p3) = self.tangent_neighbours(i);
        cubic_derivative(p0, p1, p2, p3, u, self.tension).normalize_or_zero()
    }

    /// Map a parameter t in [0, 1] to the cumulative arc length up to that
    /// point. Necessary because Catmull-Rom's t-parameterisation isn't
    /// uniform with arc length — `t * length()` is wildly wrong on curves
    /// with uneven control-point spacing (it claimed Tokyo Station was
    /// thousands of metres further along the route than it actually is).
    pub fn distance_at_t(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let mut lo = 0usize;
        let mut hi = self.arc.len() - 1;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.arc[mid].0 < t {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let (t0, d0) = self.arc[lo];
        let (t1, d1) = self.arc[hi];
        let span = (t1 - t0).max(1e-6);
        d0 + (d1 - d0) * ((t - t0) / span)
    }

    /// Map a distance along the curve to the parameter t in [0, 1].
    pub fn t_at_distance(&self, dist: f32) -> f32 {
        let d = dist.clamp(0.0, self.length);
        // Binary search in the arc table.
        let mut lo = 0usize;
        let mut hi = self.arc.len() - 1;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.arc[mid].1 < d {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let (t0, d0) = self.arc[lo];
        let (t1, d1) = self.arc[hi];
        let span = (d1 - d0).max(1e-6);
        t0 + (t1 - t0) * ((d - d0) / span)
    }

    pub fn position_at_distance(&self, dist: f32) -> Vec3 {
        self.position(self.t_at_distance(dist))
    }

    pub fn tangent_at_distance(&self, dist: f32) -> Vec3 {
        self.tangent(self.t_at_distance(dist))
    }

    fn rebuild_arc(&mut self, samples: usize) {
        self.arc.clear();
        let mut prev = self.position(0.0);
        let mut acc = 0.0;
        self.arc.push((0.0, 0.0));
        for i in 1..=samples {
            let t = i as f32 / samples as f32;
            let p = self.position(t);
            acc += (p - prev).length();
            self.arc.push((t, acc));
            prev = p;
        }
        self.length = acc;
    }

    fn segment(&self, t: f32) -> (usize, f32) {
        let t = t.clamp(0.0, 1.0);
        let n = self.points.len() - 1;
        let scaled = t * n as f32;
        let mut i = scaled.floor() as usize;
        if i >= n {
            i = n - 1;
        }
        let u = scaled - i as f32;
        (i, u)
    }

    fn tangent_neighbours(&self, i: usize) -> (Vec3, Vec3, Vec3, Vec3) {
        let n = self.points.len();
        let p1 = self.points[i];
        let p2 = self.points[i + 1];
        let p0 = if i == 0 {
            2.0 * p1 - p2
        } else {
            self.points[i - 1]
        };
        let p3 = if i + 2 >= n {
            2.0 * p2 - p1
        } else {
            self.points[i + 2]
        };
        (p0, p1, p2, p3)
    }
}

fn cubic(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, u: f32, tau: f32) -> Vec3 {
    let m1 = tau * (p2 - p0);
    let m2 = tau * (p3 - p1);
    let u2 = u * u;
    let u3 = u2 * u;
    let h00 = 2.0 * u3 - 3.0 * u2 + 1.0;
    let h10 = u3 - 2.0 * u2 + u;
    let h01 = -2.0 * u3 + 3.0 * u2;
    let h11 = u3 - u2;
    h00 * p1 + h10 * m1 + h01 * p2 + h11 * m2
}

fn cubic_derivative(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, u: f32, tau: f32) -> Vec3 {
    let m1 = tau * (p2 - p0);
    let m2 = tau * (p3 - p1);
    let u2 = u * u;
    let d00 = 6.0 * u2 - 6.0 * u;
    let d10 = 3.0 * u2 - 4.0 * u + 1.0;
    let d01 = -6.0 * u2 + 6.0 * u;
    let d11 = 3.0 * u2 - 2.0 * u;
    d00 * p1 + d10 * m1 + d01 * p2 + d11 * m2
}
