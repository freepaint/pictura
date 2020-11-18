#[test]
#[allow(clippy::float_cmp)]
fn channel_test() {
    use crate::image::Channel;

    let mut c = Channel::new(2, 2);
    assert_eq!(
        c.read_raw(),
        &[0.0, 0.0, 0.0, 0.0],
        "Channel should be zeroed"
    );
    let writeable = c.write_raw();
    writeable[1] = 0.25;
    writeable[2] = 0.5;
    writeable[3] = 0.75;
    assert_eq!(
        c.read_raw(),
        &[0.0, 0.25, 0.5, 0.75],
        "Channel should contain values, but doesn't"
    );

    // let mut iter = c.chunked_iter_mut(); // TODO: add test case here
}
