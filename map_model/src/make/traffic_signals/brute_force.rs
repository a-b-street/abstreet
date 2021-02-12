use super::*;

/// Temporary experiment to group all movements into the smallest number of stages.
pub fn make_traffic_signal(map: &Map, i: IntersectionID) {
    let movements: Vec<Movement> = Movement::for_i(i, map)
        .unwrap()
        .into_iter()
        .filter_map(|(id, m)| if id.crosswalk { None } else { Some(m) })
        .collect();
    let indices: Vec<usize> = (0..movements.len()).collect();
    for num_stages in 1..=movements.len() {
        println!(
            "For {} turn movements, looking for solution with {} stages",
            movements.len(),
            num_stages
        );
        for partition in helper(&indices, num_stages) {
            if okay_partition(movements.iter().collect(), partition) {
                return;
            }
        }
    }
    unreachable!()
}

fn okay_partition(movements: Vec<&Movement>, partition: Partition) -> bool {
    for stage in partition.0 {
        let mut protected: Vec<&Movement> = Vec::new();
        for idx in stage {
            let m = movements[idx];
            if protected.iter().any(|other| m.conflicts_with(other)) {
                return false;
            }
            protected.push(m);
        }
    }
    println!("found one that works! :O");
    true
}
