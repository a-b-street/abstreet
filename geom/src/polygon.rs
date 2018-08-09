use graphics::math::Vec2d;
use {Bounds, Pt2D};

#[derive(Debug)]
pub struct Polygon {
    // This could be stored more efficiently, but worry about it later when switching to gfx-rs.
    pub triangles: Vec<Triangle>,
}

impl Polygon {
    // Adapted from https://crates.io/crates/polygon2; couldn't use the crate directly because it
    // depends on nightly.
    pub fn new(pts: &Vec<Pt2D>) -> Polygon {
        assert!(pts.len() >= 3);

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
            .map(|tri| vec![tri.pt1.to_vec(), tri.pt2.to_vec(), tri.pt3.to_vec()])
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
            b.update_pt(&tri.pt1);
            b.update_pt(&tri.pt2);
            b.update_pt(&tri.pt3);
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
        ((self.pt1.y() - self.pt2.y()) * (self.pt3.x() - self.pt2.x())
            + (self.pt2.x() - self.pt1.x()) * (self.pt3.y() - self.pt2.y())) >= 0.0
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        let v0x = self.pt3.x() - self.pt1.x();
        let v0y = self.pt3.y() - self.pt1.y();
        let v1x = self.pt2.x() - self.pt1.x();
        let v1y = self.pt2.y() - self.pt1.y();
        let v2x = pt.x() - self.pt1.x();
        let v2y = pt.y() - self.pt1.y();

        let dot00 = v0x * v0x + v0y * v0y;
        let dot01 = v0x * v1x + v0y * v1y;
        let dot02 = v0x * v2x + v0y * v2y;
        let dot11 = v1x * v1x + v1y * v1y;
        let dot12 = v1x * v2x + v1y * v2y;

        let denom = dot00 * dot11 - dot01 * dot01;
        let u = (dot11 * dot02 - dot01 * dot12) / denom;
        let v = (dot00 * dot12 - dot01 * dot02) / denom;

        (u >= 0.0) && (v >= 0.0) && (u + v < 1.0)
    }
}
