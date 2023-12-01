use std::collections::{BTreeSet, HashMap};

use super::models::BackfillJob;

/// Assumes jobs are already sorted by from_block
pub fn rearrange(jobs: Vec<BackfillJob>, chain_id: i32) -> Vec<BackfillJob> {
    let points = jobs.iter().fold(BTreeSet::new(), |mut acc, j| {
        acc.insert(j.from_block);
        // adds one to represent this as a non-inclusive range,
        // otherwise we lose info
        // (e.g: the range 3..=3 disappears since we remove duplicates)
        acc.insert(j.to_block + 1);
        acc
    });

    let sorted_points: Vec<i32> = points.into_iter().collect();

    let mut range_map = HashMap::new();
    let mut size = 0;

    for i in 0..sorted_points.len() - 1 {
        let start = sorted_points[i];
        let end = sorted_points[i + 1];

        if start <= end {
            let mut addresses = Vec::new();
            for job in jobs.iter() {
                if job.from_block > end {
                    break;
                };
                if job.from_block <= start && job.to_block >= end - 1 {
                    addresses.push(job.address.0)
                }
            }

            size += addresses.len();
            // convert back to inclusive range, which is the representation used
            // outside this algo
            range_map.insert((start, end - 1), addresses);
        }
    }

    let mut res = Vec::with_capacity(size);
    range_map
        .into_iter()
        .for_each(|((from_block, to_block), addresses)| {
            addresses.into_iter().for_each(|address| {
                res.push(BackfillJob {
                    address: address.into(),
                    chain_id,
                    from_block,
                    to_block,
                })
            })
        });

    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;
    use rstest::*;

    type Mock = (u8, i32, i32);
    type Expectation = (Vec<u8>, i32, i32);

    #[rstest]
    #[case(vec![(0x1, 1, 2), (0x2, 1, 3)], vec![(vec![0x1, 0x2], 1, 2), (vec![0x2], 3, 3)])]
    #[case(vec![(0x1, 1, 10), (0x2, 5, 15)], vec![(vec![0x1], 1, 4), (vec![0x1, 0x2], 5, 10), (vec![0x2], 11, 15)])]
    #[case(vec![(0x1, 1, 1), (0x2, 2, 2), (0x3, 3, 3)], vec![(vec![0x1], 1, 1), (vec![0x2], 2, 2), (vec![0x3], 3, 3)])]
    fn test(#[case] ranges: Vec<Mock>, #[case] expected: Vec<Expectation>) {
        let ranges = ranges_to_jobs(ranges);
        dbg!(&ranges);
        let result = rearrange(ranges, 1);
        dbg!(&result);

        compare(result, expected);
    }

    fn ranges_to_jobs(ranges: Vec<(u8, i32, i32)>) -> Vec<BackfillJob> {
        ranges
            .into_iter()
            .map(|(addr, from_block, to_block)| {
                let slice = &[addr; 20];
                let address = Address::from_slice(slice).into();
                BackfillJob {
                    from_block,
                    to_block,
                    address,
                    chain_id: 1,
                }
            })
            .collect()
    }

    // compares the rearranged results with expectation
    fn compare(mut result: Vec<BackfillJob>, mut expected: Vec<Expectation>) {
        while let Some(job) = result.pop() {
            if let Some(ref mut e) = expected.iter_mut().find(|ref e| {
                job.from_block == e.1
                    && job.to_block == e.2
                    && e.0.contains(&job.address.0.as_slice()[0])
            }) {
                if let Some(i) = e.0.iter().position(|a| *a == job.address.0[0]) {
                    e.0.swap_remove(i);
                    continue;
                }
            }

            panic!(
                "Expected to find ({}, {}, {})",
                job.address.0, job.from_block, job.to_block
            );
        }

        assert!(expected.iter().all(|e| e.0.is_empty()));
    }
}
