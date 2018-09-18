use graphics::math::Vec2d;
use {Bounds, Pt2D};

#[derive(Debug)]
pub struct Polygon {
    // This could be stored more efficiently, but worry about it later when switching to gfx-rs.
    pub triangles: Vec<Triangle>,
}

// TODO The triangulation is a bit of a mess. Everything except for Polygon::new comes from
// https://github.com/lionfish0/earclip/blob/master/earclip/__init__.py.

impl Polygon {
    // TODO Should the first and last points match or not?
    // Adapted from https://crates.io/crates/polygon2; couldn't use the crate directly because it
    // depends on nightly.
    pub fn new(orig_pts: &Vec<Pt2D>) -> Polygon {
        assert!(orig_pts.len() >= 3);

        let pts = if is_clockwise_polygon(orig_pts) {
            let mut new_pts = orig_pts.clone();
            new_pts.reverse();
            new_pts
        } else {
            orig_pts.clone()
        };

        let mut tgs = Vec::new();
        let mut avl = Vec::with_capacity(pts.len());
        for i in 0..pts.len() {
            avl.push(i);
        }

        let mut i = 0;
        let mut al = pts.len();
        while al > 3 {
            let i0 = avl[i % al];
            let i1 = avl[(i + 1) % al];
            let i2 = avl[(i + 2) % al];

            let a = pts[i0];
            let b = pts[i1];
            let c = pts[i2];
            let tri = Triangle::new(a, b, c);

            let mut ear_found = false;
            if tri.is_convex() {
                ear_found = true;

                for j in 0..al {
                    let vi = avl[j];

                    if vi != i0 && vi != i1 && vi != i2 {
                        if tri.contains_pt(pts[vi]) {
                            ear_found = false;
                            break;
                        }
                    }
                }
            }

            if ear_found {
                tgs.push(i0);
                tgs.push(i1);
                tgs.push(i2);
                avl.remove((i + 1) % al);
                al -= 1;
                i = 0;
            } else if i > 3 * al {
                break;
            } else {
                i += 1;
            }
        }

        tgs.push(avl[0]);
        tgs.push(avl[1]);
        tgs.push(avl[2]);

        let mut triangles = Vec::new();
        assert!(tgs.len() % 3 == 0);
        for tri in tgs.chunks(3) {
            triangles.push(Triangle::new(pts[tri[0]], pts[tri[1]], pts[tri[2]]));
        }
        Polygon { triangles }
    }

    pub fn for_drawing(&self) -> Vec<Vec<Vec2d>> {
        self.triangles
            .iter()
            .map(|tri| vec![to_vec(tri.pt1), to_vec(tri.pt2), to_vec(tri.pt3)])
            .collect()
    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        self.triangles
            .iter()
            .find(|tri| tri.contains_pt(pt))
            .is_some()
    }

    pub fn get_bounds(&self) -> Bounds {
        let mut b = Bounds::new();
        for tri in &self.triangles {
            b.update_pt(tri.pt1);
            b.update_pt(tri.pt2);
            b.update_pt(tri.pt3);
        }
        b
    }
}

#[derive(Debug)]
pub struct Triangle {
    pub pt1: Pt2D,
    pub pt2: Pt2D,
    pub pt3: Pt2D,
}

impl Triangle {
    pub(crate) fn new(pt1: Pt2D, pt2: Pt2D, pt3: Pt2D) -> Triangle {
        Triangle { pt1, pt2, pt3 }
    }

    fn is_convex(&self) -> bool {
        let x1 = self.pt1.x();
        let y1 = self.pt1.y();
        let x2 = self.pt2.x();
        let y2 = self.pt2.y();
        let x3 = self.pt3.x();
        let y3 = self.pt3.y();

        let cross_product = (x2 - x1) * (y3 - y1) - (y2 - y1) * (x3 - x1);
        cross_product >= 0.0
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        let x1 = self.pt1.x();
        let y1 = self.pt1.y();
        let x2 = self.pt2.x();
        let y2 = self.pt2.y();
        let x3 = self.pt3.x();
        let y3 = self.pt3.y();
        let px = pt.x();
        let py = pt.y();

        // Barycentric coefficients for pt
        // Use epsilon to deal with small denominators
        let epsilon = 0.0000001;
        let l0 = ((y2 - y3) * (px - x3) + (x3 - x2) * (py - y3))
            / (((y2 - y3) * (x1 - x3) + (x3 - x2) * (y1 - y3)) + epsilon);
        let l1 = ((y3 - y1) * (px - x3) + (x1 - x3) * (py - y3))
            / (((y2 - y3) * (x1 - x3) + (x3 - x2) * (y1 - y3)) + epsilon);
        let l2 = 1.0 - l0 - l1;

        for x in vec![l0, l1, l2] {
            if x >= 1.0 || x <= 0.0 {
                return false;
            }
        }
        true
    }
}

fn is_clockwise_polygon(pts: &Vec<Pt2D>) -> bool {
    // Initialize with the last element
    let mut sum = (pts[0].x() - pts.last().unwrap().x()) * (pts[0].y() + pts.last().unwrap().y());
    for i in 0..pts.len() - 1 {
        sum += (pts[i + 1].x() - pts[i].x()) * (pts[i + 1].y() + pts[i].y());
    }
    sum > 0.0
}

fn to_vec(pt: Pt2D) -> Vec2d {
    [pt.x(), pt.y()]
}
