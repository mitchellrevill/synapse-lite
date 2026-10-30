//! The native effect-local stream, separate from Fire's CRT random stream.
pub(crate) fn next(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(214013).wrapping_add(2531011);
    (*state >> 16) ^ *state
}

pub(crate) fn color(state: &mut u32) -> u32 {
    loop {
        let channels = std::array::from_fn::<_, 3, _>(|_| match next(state) % 255 {
            171.. => 255u8,
            86..=170 => 128,
            _ => 0,
        });
        if channels[0] != channels[1] || channels[1] != channels[2] {
            // Native helper writes BGR; saved Studio colors use RGB24.
            return u32::from(channels[2]) << 16
                | u32::from(channels[1]) << 8
                | u32::from(channels[0]);
        }
    }
}
