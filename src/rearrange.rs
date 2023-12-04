use std::collections::{BTreeSet, HashMap};

use crate::db::models::BackfillJob;

/// Assumes jobs are already sorted by from_block
pub fn rearrange(jobs: &[BackfillJob]) -> Vec<BackfillJob> {
    dbg!(&jobs);
    let points = jobs
        .iter()
        .filter(|j| j.low != j.high) // filter out empty jobs
        .fold(BTreeSet::new(), |mut acc, j| {
            acc.insert(j.low);
            acc.insert(j.high);
            acc
        });

    let sorted_points: Vec<i32> = points.into_iter().collect();

    dbg!(&sorted_points);

    let mut range_map = HashMap::new();
    let mut size = 0;

    for i in 0..sorted_points.len().saturating_sub(1) {
        let start = sorted_points[i];
        let end = sorted_points[i + 1];
        let range = start..end;

        println!();
        println!();
        println!();
        println!("{:?}", start..end);
        let mut addresses = Vec::new();
        for job in jobs.iter() {
            println!("{:?}", job.addresses[0]);
            if job.low >= end {
                println!("break");
                break;
            };

            let job_range = job.low..job.high;

            if job_range.contains(&range.start) && job_range.contains(&(range.end - 1)) {
                // }
                // println!("{:?}", job.low..job.high);
                //
                // if dbg!(range.contains(&job.low)) && dbg!(range.contains(&(job.high - 1))) {
                println!("include");
                addresses.extend_from_slice(&job.addresses)
            }
        }

        size += addresses.len();
        if !addresses.is_empty() {
            range_map.insert((start, end), addresses);
        }
        println!();
    }

    dbg!(&range_map);
    let mut res = Vec::with_capacity(size);
    range_map.into_iter().for_each(|((low, high), addresses)| {
        res.push(BackfillJob {
            addresses,
            low,
            high,
        })
    });
    dbg!(&res);

    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;
    use rstest::*;

    #[derive(Debug, PartialEq)]
    struct FakeJob(Vec<u8>, i32, i32);

    #[derive(Debug)]
    struct Fixture {
        input: Vec<FakeJob>,
        output: Vec<FakeJob>,
    }

    #[fixture]
    fn adjacent_jobs_1() -> Fixture {
        Fixture {
            input: vec![FakeJob(vec![0x1], 0, 10), FakeJob(vec![0x1], 10, 20)],
            output: vec![FakeJob(vec![0x1], 0, 10), FakeJob(vec![0x1], 10, 20)],
        }
    }

    #[fixture]
    fn same_range_different_addresses() -> Fixture {
        Fixture {
            input: vec![FakeJob(vec![0x1], 0, 10), FakeJob(vec![0x2], 0, 10)],
            output: vec![FakeJob(vec![0x1, 0x2], 0, 10)],
        }
    }

    #[fixture]
    fn empty_range() -> Fixture {
        Fixture {
            input: vec![FakeJob(vec![0x1], 0, 0)],
            output: vec![],
        }
    }

    #[fixture]
    fn single_block() -> Fixture {
        Fixture {
            input: vec![FakeJob(vec![0x1], 0, 1)],
            output: vec![FakeJob(vec![0x1], 0, 1)],
        }
    }

    #[fixture]
    fn mix1() -> Fixture {
        Fixture {
            input: vec![FakeJob(vec![0x1], 1, 2), FakeJob(vec![0x2], 1, 3)],
            output: vec![FakeJob(vec![0x1, 0x2], 1, 2), FakeJob(vec![0x2], 2, 3)],
        }
    }

    #[fixture]
    fn mix2() -> Fixture {
        Fixture {
            input: vec![FakeJob(vec![0x1], 1, 10), FakeJob(vec![0x2], 5, 15)],
            output: vec![
                FakeJob(vec![0x1], 1, 5),
                FakeJob(vec![0x1, 0x2], 5, 10),
                FakeJob(vec![0x2], 10, 15),
            ],
        }
    }

    #[fixture]
    fn mix3() -> Fixture {
        Fixture {
            input: vec![
                FakeJob(vec![0x1], 10, 20),
                FakeJob(vec![0x2], 15, 25),
                FakeJob(vec![0x3], 20, 30),
            ],
            output: vec![
                FakeJob(vec![0x1], 10, 15),
                FakeJob(vec![0x1, 0x2], 15, 20),
                // FakeJob(vec![0x1, 0x2, 0x3], 20, 20),
                FakeJob(vec![0x2, 0x3], 20, 25),
                FakeJob(vec![0x3], 25, 30),
            ],
        }
    }

    #[fixture]
    fn mix4() -> Fixture {
        Fixture {
            input: vec![
                FakeJob(vec![0x1], 10, 21),
                FakeJob(vec![0x2], 15, 25),
                FakeJob(vec![0x3], 20, 30),
            ],
            output: vec![
                FakeJob(vec![0x1], 10, 15),
                FakeJob(vec![0x1, 0x2], 15, 20),
                FakeJob(vec![0x1, 0x2, 0x3], 20, 21),
                FakeJob(vec![0x2, 0x3], 21, 25),
                FakeJob(vec![0x3], 25, 30),
            ],
        }
    }

    #[rstest]
    #[case(adjacent_jobs_1())]
    #[case(same_range_different_addresses())]
    #[case(empty_range())]
    #[case(single_block())]
    #[case(mix1())]
    #[case(mix2())]
    #[case(mix3())]
    #[case(mix4())]
    fn test(#[case] fixture: Fixture) {
        let jobs = to_jobs(fixture.input);
        let mut result = rearrange(&jobs);
        result.sort_by(|j, j2| j.low.cmp(&j2.low));

        dbg!(&result);
        assert_eq!(result.len(), fixture.output.len());

        for (job, expectation) in result.into_iter().zip(fixture.output.iter()) {
            let fake = FakeJob(
                job.addresses
                    .into_iter()
                    .map(|a| a.0.as_slice()[0])
                    .collect(),
                job.low,
                job.high,
            );
            assert_eq!(&fake, expectation);
        }
    }

    fn to_jobs(ranges: Vec<FakeJob>) -> Vec<BackfillJob> {
        ranges
            .into_iter()
            .map(|FakeJob(ids, low, high)| {
                let addresses = ids
                    .into_iter()
                    .map(|i| {
                        let slice = &[i; 20];
                        Address::from_slice(slice).into()
                    })
                    .collect();

                BackfillJob {
                    low,
                    high,
                    addresses,
                }
            })
            .collect()
    }
}
