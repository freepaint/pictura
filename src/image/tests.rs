#[test]
#[allow(clippy::float_cmp)]
fn channel_test_st() {
    use crate::image::Channel;

    let mut c = Channel::new(2, 2);
    assert_eq!(c.locked(), 0);
    assert_eq!(
        c.lock_read().read_raw(),
        &[0.0, 0.0, 0.0, 0.0],
        "Channel should be zeroed"
    );
    assert_eq!(c.locked(), 0);
    {
        let mut lock = c.lock_write();
        let writeable = lock.write_raw();
        writeable[1] = 0.25;
        writeable[2] = 0.5;
        writeable[3] = 0.75;
    }
    assert_eq!(c.locked(), 0);
    assert_eq!(
        c.lock_read().read_raw(),
        &[0.0, 0.25, 0.5, 0.75],
        "Channel should contain values, but doesn't"
    );
    assert_eq!(c.locked(), 0);

    // let mut iter = c.chunked_iter_mut(); // TODO: add test case here
}

#[test]
#[allow(clippy::float_cmp)]
fn channel_test_mt() {
    use crate::image::Channel;

    let mut c = Channel::new(2, 2);

    let bar = std::sync::Arc::new(std::sync::Barrier::new(3));

    {
        let mut wl = c.lock_write();
        wl.chunked_iter_mut().for_each(|mut block| {
            let bc = bar.clone();
            std::thread::spawn(move || {
                let ofs = block.offset();
                block.get_mut()[0] = ofs.m11 as f32 + ofs.m12 as f32 * 2.0;
                block.get_mut()[1] = ofs.m11 as f32 + ofs.m12 as f32 * 2.0 + 1.0;
                bc.wait();
            });
        })
    }
    bar.wait();
    assert_eq!(
        c.lock_read().read_raw(),
        &[0.0, 1.0, 2.0, 3.0],
        "Channel should contain values, but doesn't"
    );
}

#[test]
fn layer_test_gray() {
    use crate::image::Layer;

    let layer = Layer::new_gray(100, 100);
    let channel = layer.channel();

    assert!(
        channel.contains(&"gray".to_string()),
        "Layer 'gray' is missing!"
    );
    assert_eq!(channel.len(), 1, "To many channels? {:?}", channel);
}

#[test]
fn layer_test_rgb() {
    use crate::image::Layer;

    let layer = Layer::new_rgb(100, 100);
    let channel = layer.channel();

    assert!(
        channel.contains(&"red".to_string()),
        "Layer 'red' is missing!"
    );
    assert!(
        channel.contains(&"green".to_string()),
        "Layer 'green' is missing!"
    );
    assert!(
        channel.contains(&"blue".to_string()),
        "Layer 'blue' is missing!"
    );
    assert_eq!(channel.len(), 3, "To many channels? {:?}", channel);
}

#[test]
fn layer_test_rgba() {
    use crate::image::Layer;

    let layer = Layer::new_rgba(100, 100);
    let channel = layer.channel();

    assert!(
        channel.contains(&"red".to_string()),
        "Layer 'red' is missing!"
    );
    assert!(
        channel.contains(&"green".to_string()),
        "Layer 'green' is missing!"
    );
    assert!(
        channel.contains(&"blue".to_string()),
        "Layer 'blue' is missing!"
    );
    assert!(
        channel.contains(&"alpha".to_string()),
        "Layer 'alpha' is missing!"
    );
    assert_eq!(channel.len(), 4, "To many channels? {:?}", channel);
}
