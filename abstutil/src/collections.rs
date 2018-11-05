pub fn wraparound_get<T>(vec: &Vec<T>, idx: isize) -> &T {
    let len = vec.len() as isize;
    let idx = idx % len;
    let idx = if idx >= 0 { idx } else { idx + len };
    &vec[idx as usize]
}
