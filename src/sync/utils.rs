use alloy_primitives::{Address, FixedBytes};

pub(super) fn topic_as_address(topic: &FixedBytes<32>) -> Option<Address> {
    let padding_slice = &topic.as_slice()[0..12];
    let padding: FixedBytes<12> = FixedBytes::from_slice(padding_slice);

    if padding.is_zero() {
        Some(Address::from_slice(&topic[12..]))
    } else {
        None
    }
}
