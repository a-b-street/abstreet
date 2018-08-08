use graphics::math::Vec2d;
use Pt2D;

// Adapted from https://crates.io/crates/polygon2; couldn't use the crate directly because it
// depends on nightly.

pub fn triangulate(pts: &Vec<Pt2D>) -> Vec<Vec<Vec2d>> {
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

        let mut ear_found = false;
        if is_triangle_convex(a, b, c) {
            ear_found = true;

            for j in 0..al {
                let vi = avl[j];

                if vi != i0 && vi != i1 && vi != i2 {
                    if point_in_triangle(pts[vi], a, b, c) {
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

    let mut result = Vec::new();
    assert!(tgs.len() % 3 == 0);
    for tri in tgs.chunks(3) {
        result.push(vec![
            pts[tri[0]].to_vec(),
            pts[tri[1]].to_vec(),
            pts[tri[2]].to_vec(),
        ]);
    }

    result
}

fn is_triangle_convex(a: Pt2D, b: Pt2D, c: Pt2D) -> bool {
    ((a.y() - b.y()) * (c.x() - b.x()) + (b.x() - a.x()) * (c.y() - b.y())) >= 0.0
}

fn point_in_triangle(p: Pt2D, a: Pt2D, b: Pt2D, c: Pt2D) -> bool {
    let v0x = c.x() - a.x();
    let v0y = c.y() - a.y();
    let v1x = b.x() - a.x();
    let v1y = b.y() - a.y();
    let v2x = p.x() - a.x();
    let v2y = p.y() - a.y();

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
