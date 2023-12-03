use std::collections::{BTreeSet, HashMap};

use crate::db::models::BackfillJob;

/// Assumes jobs are already sorted by from_block
pub fn rearrange(jobs: &[BackfillJob]) -> Vec<BackfillJob> {
    let points = jobs
        .iter()
        .filter(|j| j.low == j.high) // filter out empty jobs
        .fold(BTreeSet::new(), |mut acc, j| {
            acc.insert(j.low);
            acc.insert(j.high);
            acc
        });

    let sorted_points: Vec<i32> = points.into_iter().collect();

    let mut range_map = HashMap::new();
    let mut size = 0;

    for i in 0..sorted_points.len().saturating_sub(1) {
        let start = sorted_points[i];
        let end = sorted_points[i + 1];

        if start <= end {
            let mut addresses = Vec::new();
            for job in jobs.iter() {
                if job.low > end {
                    break;
                };

                if job.low <= start && job.high >= end - 1 {
                    addresses.extend_from_slice(&job.addresses)
                }
            }

            size += addresses.len();
            range_map.insert((start, end), addresses);
        }
    }

    let mut res = Vec::with_capacity(size);
    range_map.into_iter().for_each(|((low, high), addresses)| {
        res.push(BackfillJob {
            addresses,
            low,
            high,
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
    #[case(vec![(0x1, 1, 1)], vec![])]
    #[case(vec![(0x1, 1, 2), (0x2, 1, 3)], vec![(vec![0x1, 0x2], 1, 2), (vec![0x2], 3, 3)])]
    #[case(vec![(0x1, 1, 10), (0x2, 5, 15)], vec![(vec![0x1], 1, 4), (vec![0x1, 0x2], 5, 10), (vec![0x2], 11, 15)])]
    #[case(vec![(0x1, 1, 1), (0x2, 2, 2), (0x3, 3, 3)], vec![(vec![0x1], 1, 1), (vec![0x2], 2, 2), (vec![0x3], 3, 3)])]
    #[case(vec![(0x1, 10, 20), (0x2, 15, 25), (0x3, 20, 30)], vec![(vec![0x1], 10, 14), (vec![0x1, 0x2], 15, 19), (vec![0x1, 0x2, 0x3], 20, 20), (vec![0x2, 0x3], 21, 25), (vec![0x3], 26, 30)])]
    fn test(#[case] ranges: Vec<Mock>, #[case] expected: Vec<Expectation>) {
        let ranges = ranges_to_jobs(ranges);
        let result = rearrange(&ranges);

        compare(result, expected);
    }

    fn ranges_to_jobs(ranges: Vec<(u8, i32, i32)>) -> Vec<BackfillJob> {
        ranges
            .into_iter()
            .map(|(addr, low, high)| {
                let slice = &[addr; 20];
                let address = Address::from_slice(slice).into();
                BackfillJob {
                    low,
                    high,
                    addresses: vec![address],
                }
            })
            .collect()
    }

    // compares the rearranged results with expectation
    fn compare(mut result: Vec<BackfillJob>, mut expected: Vec<Expectation>) {
        while let Some(job) = result.pop() {
            if let Some(ref mut e) = expected
                .iter_mut()
                .find(|e| job.low == e.1 && job.high == e.2)
            {
                assert_eq!(job.addresses.len(), e.0.len());
                e.0.iter()
                    .zip(job.addresses.iter())
                    .for_each(|(a, b)| assert_eq!(a, &b.0.as_slice()[0]));
            }
        }
    }
}
