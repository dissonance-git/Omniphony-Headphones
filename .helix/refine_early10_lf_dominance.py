from pathlib import Path

p = Path('omniphony-renderer/orender_engine/src/music_early_reflections.rs')
s = p.read_text()

def once(old: str, new: str, label: str) -> None:
    global s
    n = s.count(old)
    if n != 1:
        raise RuntimeError(f'{label}: expected one anchor, found {n}')
    s = s.replace(old, new, 1)

once(
    'const TRANSIENT_DETECT_LP_HZ: f32 = 180.0;\n',
    'const TRANSIENT_DETECT_LP_HZ: f32 = 180.0;\nconst TRANSIENT_MIN_LF_SHARE: f32 = 0.35;\n',
    'LF dominance threshold',
)

once(
'''    low_state_2: f32,
    low_alpha: f32,
}''',
'''    low_state_2: f32,
    low_alpha: f32,
    broadband_fast_energy: f32,
}''',
    'broadband reference field',
)

once(
'''            low_alpha: 1.0
                - (-std::f32::consts::TAU * TRANSIENT_DETECT_LP_HZ / sample_rate_hz).exp(),
        }''',
'''            low_alpha: 1.0
                - (-std::f32::consts::TAU * TRANSIENT_DETECT_LP_HZ / sample_rate_hz).exp(),
            broadband_fast_energy: 0.0,
        }''',
    'broadband reference init',
)

once(
'''        let energy = low * low;
        self.fast_energy += self.fast_alpha * (energy - self.fast_energy);
        self.slow_energy += self.slow_alpha * (energy - self.slow_energy);

        let target = if self.fast_energy > TRANSIENT_MIN_RMS * TRANSIENT_MIN_RMS {
            let positive_rise = (self.fast_energy - self.slow_energy).max(0.0);
            let relative_rise = positive_rise / (self.slow_energy + 1.0e-9);
            ((relative_rise - TRANSIENT_RISE_THRESHOLD)
                / (TRANSIENT_FULL_RISE - TRANSIENT_RISE_THRESHOLD))
                .clamp(0.0, 1.0)
        } else {
            0.0
        };''',
'''        let energy = low * low;
        self.fast_energy += self.fast_alpha * (energy - self.fast_energy);
        self.slow_energy += self.slow_alpha * (energy - self.slow_energy);
        let broadband_energy = input * input;
        self.broadband_fast_energy +=
            self.fast_alpha * (broadband_energy - self.broadband_fast_energy);
        let lf_share = self.fast_energy / (self.broadband_fast_energy + 1.0e-9);

        let target = if self.fast_energy > TRANSIENT_MIN_RMS * TRANSIENT_MIN_RMS
            && lf_share >= TRANSIENT_MIN_LF_SHARE
        {
            let positive_rise = (self.fast_energy - self.slow_energy).max(0.0);
            let relative_rise = positive_rise / (self.slow_energy + 1.0e-9);
            ((relative_rise - TRANSIENT_RISE_THRESHOLD)
                / (TRANSIENT_FULL_RISE - TRANSIENT_RISE_THRESHOLD))
                .clamp(0.0, 1.0)
        } else {
            0.0
        };''',
    'LF-dominant target gate',
)

p.write_text(s)
print('refined', p)
