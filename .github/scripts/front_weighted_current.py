from pathlib import Path

p = Path('omniphony-renderer/renderer/src/music_field.rs')
s = p.read_text()

old = '''/// The native Windows listening pass deliberately increases only the two bands
/// carrying the strongest elevation cues. Research repeatedly places useful
/// vertical spectral structure through roughly 2-10 kHz, so the 320-1200 Hz
/// musical-body transfer is retained while 1.2-5 kHz and >5 kHz move modestly
/// higher. Front remains stronger than rear to raise the perceived ceiling
/// without hollowing the lateral/rear wrap.
const FRONT_COHERENT_HEIGHT_TRANSFER: [f32; 3] = [0.22, 0.54, 0.44];
const REAR_COHERENT_HEIGHT_TRANSFER: [f32; 3] = [0.12, 0.32, 0.28];
'''
new = '''/// Current keeps height, but the horizontal front must remain load-bearing.
/// Physical listening found the previous front transfer could leave the lower
/// front hemisphere perceptually hollow while the side/rear/top shell remained
/// convincing. Reclaim a modest amount of the presence/high-band front support
/// from elevation instead of adding a second copy.
const FRONT_COHERENT_HEIGHT_TRANSFER: [f32; 3] = [0.18, 0.44, 0.36];
const REAR_COHERENT_HEIGHT_TRANSFER: [f32; 3] = [0.12, 0.32, 0.28];

/// Forward shell weighting. These are transfers, not gains: an existing piece
/// of rear support is moved to the matching front lane sample-for-sample. The
/// protected stereo master remains the center/vocal authority; stereo does not
/// synthesize a fake authored C channel. The result should feel like a front arc
/// wrapping around the master rather than a bubble whose center of mass sits
/// behind the listener.
const REAR_TO_FRONT_TRANSFER: [f32; 3] = [0.10, 0.16, 0.12];
const TOP_REAR_TO_TOP_FRONT_TRANSFER: [f32; 3] = [0.06, 0.10, 0.08];
'''
assert old in s, 'height constants baseline drifted'
s = s.replace(old, new, 1)

old = '''#[inline]
fn transfer_to_elevation(horizontal: &mut f32, elevated: &mut f32, fraction: f32) {
    let transfer = *horizontal * fraction.clamp(0.0, 0.60);
    *horizontal -= transfer;
    *elevated += transfer;
}
'''
new = old + '''
#[inline]
fn transfer_forward(rear: &mut f32, front: &mut f32, fraction: f32) {
    let transfer = *rear * fraction.clamp(0.0, 0.40);
    *rear -= transfer;
    *front += transfer;
}
'''
assert old in s, 'elevation transfer helper baseline drifted'
s = s.replace(old, new, 1)

old = '''                transfer_to_elevation(&mut band_front_l, &mut band_top_front_l, front_transfer);
                transfer_to_elevation(&mut band_front_r, &mut band_top_front_r, front_transfer);
                transfer_to_elevation(&mut band_rear_l, &mut band_top_rear_l, rear_transfer);
                transfer_to_elevation(&mut band_rear_r, &mut band_top_rear_r, rear_transfer);

                front_l += band_front_l;
'''
new = '''                transfer_to_elevation(&mut band_front_l, &mut band_top_front_l, front_transfer);
                transfer_to_elevation(&mut band_front_r, &mut band_top_front_r, front_transfer);
                transfer_to_elevation(&mut band_rear_l, &mut band_top_rear_l, rear_transfer);
                transfer_to_elevation(&mut band_rear_r, &mut band_top_rear_r, rear_transfer);

                // Shift the shell's center of mass forward without creating
                // another spatial copy. Horizontal and elevated rear support
                // each keep most of their energy, so envelopment survives while
                // the front hemisphere becomes a stronger frame around the
                // protected center/master.
                let forward = REAR_TO_FRONT_TRANSFER[band - 1];
                let top_forward = TOP_REAR_TO_TOP_FRONT_TRANSFER[band - 1];
                transfer_forward(&mut band_rear_l, &mut band_front_l, forward);
                transfer_forward(&mut band_rear_r, &mut band_front_r, forward);
                transfer_forward(&mut band_top_rear_l, &mut band_top_front_l, top_forward);
                transfer_forward(&mut band_top_rear_r, &mut band_top_front_r, top_forward);

                front_l += band_front_l;
'''
assert old in s, 'support routing baseline drifted'
s = s.replace(old, new, 1)

old = '''    #[test]
    fn native_height_polish_targets_mid_and_high_without_lifting_body_band() {
        assert_eq!(FRONT_COHERENT_HEIGHT_TRANSFER[0], 0.22);
        assert!(FRONT_COHERENT_HEIGHT_TRANSFER[1] > 0.46);
        assert!(FRONT_COHERENT_HEIGHT_TRANSFER[2] > 0.38);
        assert!(REAR_COHERENT_HEIGHT_TRANSFER[1] < FRONT_COHERENT_HEIGHT_TRANSFER[1]);
        assert!(REAR_COHERENT_HEIGHT_TRANSFER[2] < FRONT_COHERENT_HEIGHT_TRANSFER[2]);
    }
'''
new = '''    #[test]
    fn native_height_polish_keeps_horizontal_front_load_bearing() {
        assert_eq!(FRONT_COHERENT_HEIGHT_TRANSFER[0], 0.18);
        assert!(FRONT_COHERENT_HEIGHT_TRANSFER[1] >= 0.40);
        assert!(FRONT_COHERENT_HEIGHT_TRANSFER[2] >= 0.32);
        assert!(REAR_COHERENT_HEIGHT_TRANSFER[1] < FRONT_COHERENT_HEIGHT_TRANSFER[1]);
        assert!(REAR_COHERENT_HEIGHT_TRANSFER[2] < FRONT_COHERENT_HEIGHT_TRANSFER[2]);
    }

    #[test]
    fn front_weighting_moves_existing_support_without_adding_a_copy() {
        for rear in [0.75_f32, -0.75, 0.125, -0.125] {
            let mut r = rear;
            let mut f = 0.20_f32;
            let before = r + f;
            transfer_forward(&mut r, &mut f, 0.16);
            assert!((r + f - before).abs() < 1.0e-6);
            assert!(r.abs() < rear.abs());
        }
        for value in REAR_TO_FRONT_TRANSFER
            .into_iter()
            .chain(TOP_REAR_TO_TOP_FRONT_TRANSFER)
        {
            assert!((0.0..=0.20).contains(&value));
        }
    }
'''
assert old in s, 'height test baseline drifted'
s = s.replace(old, new, 1)

p.write_text(s)
