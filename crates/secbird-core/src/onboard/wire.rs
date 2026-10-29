//! Static Device196 wire contract. No profile names, GUIDs or mapping packets
//! are synthesized. GET stage IDs are one-based; SET IDs are zero-based.
use super::*;
const PROFILE_POLLING: Command = Command::new(0, 0x0e);
const STAGES_SIZE: usize = 80;
fn bad() -> Error {
    Error::InvalidArg("Malformed onboard register payload; recovery record retained".into())
}
pub(super) fn validate(s: &Settings) -> Result<()> {
    if s.dpi.len() != 7
        || s.stages.len() != STAGES_SIZE
        || s.polling.len() != 2
        || s.dpi[0] != 1
        || s.stages[0] != 1
        || s.polling[0] != 1
        || super::polling_hz(s.polling[1]).is_none()
    {
        return Err(bad());
    }
    let n = usize::from(s.stages[2]);
    if !(1..=5).contains(&n) || !(1..=n as u8).contains(&s.stages[1]) {
        return Err(bad());
    }
    if (0..n).any(|i| s.stages[3 + 7 * i] != i as u8 + 1) {
        return Err(bad());
    }
    Ok(())
}
pub(super) fn project_polling(s: &mut Settings, hz: u32) -> Result<()> {
    // Device196's source profile rates; 250 and HyperPolling are not advertised.
    if ![125, 500, 1000].contains(&hz) {
        return Err(Error::InvalidArg(
            "Onboard polling supports 125, 500 or 1000 Hz for this family".into(),
        ));
    }
    s.polling[1] = cmd::polling_code(hz).ok_or_else(bad)?;
    Ok(())
}
pub(super) fn project_dpi(s: &mut Settings, stages: &DpiStages) -> Result<()> {
    let n = stages.stages.len();
    if !(1..=5).contains(&n) || !(1..=n as u8).contains(&stages.active) {
        return Err(bad());
    }
    // The selected source advertises 100..20000 in 50-DPI steps. Preserve raw
    // originals above that range, but do not manufacture unsupported new intent.
    if stages.stages.iter().any(|&(x, y)| {
        [x, y]
            .iter()
            .any(|v| !(100..=20000).contains(v) || v % 50 != 0)
    }) {
        return Err(Error::InvalidArg(
            "Source onboard DPI range is 100–20000 in steps of 50".into(),
        ));
    }
    s.stages[1] = stages.active;
    s.stages[2] = n as u8;
    for (i, &(x, y)) in stages.stages.iter().enumerate() {
        let start = 3 + i * 7;
        let [xh, xl] = x.to_be_bytes();
        let [yh, yl] = y.to_be_bytes();
        // Source authored rows use Z=0; raw originals remain in the journal.
        s.stages[start..start + 7].copy_from_slice(&[i as u8 + 1, xh, xl, yh, yl, 0, 0]);
    }
    let (x, y) = stages.stages[usize::from(stages.active - 1)];
    s.dpi[1..3].copy_from_slice(&x.to_be_bytes());
    s.dpi[3..5].copy_from_slice(&y.to_be_bytes());
    s.dpi[5..7].fill(0);
    Ok(())
}
pub(super) fn read(dev: &Device) -> Result<Settings> {
    Ok(Settings {
        dpi: dev.get(cmd::DPI, 7, &[1])?.args()[..7].to_vec(),
        stages: dev.get(cmd::DPI_STAGES, 80, &[1])?.args().to_vec(),
        polling: dev.get(PROFILE_POLLING, 2, &[1])?.args()[..2].to_vec(),
    })
}
pub(super) fn write_verified(
    dev: &Device,
    want: &Settings,
    restoring: bool,
    skip_dpi: bool,
) -> Result<()> {
    validate(want)?;
    let n = 3 + usize::from(want.stages[2]) * 7;
    let mut stages = want.stages[..n].to_vec();
    for i in 0..usize::from(want.stages[2]) {
        stages[3 + 7 * i] -= 1;
    }
    if !restoring && !skip_dpi {
        dev.transact(cmd::DPI, 7, &want.dpi)?;
    }
    if !skip_dpi {
        dev.transact(cmd::DPI_STAGES, n as u8, &stages)?;
    }
    // Restoration must also recover active DPI independently of its stage.
    if restoring {
        dev.transact(cmd::DPI, 7, &want.dpi)?;
    }
    dev.transact(PROFILE_POLLING, 2, &want.polling)?;
    let actual = read(dev)?;
    validate(&actual)?;
    if !want.matches_readback(&actual) {
        return Err(Error::Unsupported(
            "Onboard settings readback differs; originals remain saved for recovery".into(),
        ));
    }
    Ok(())
}
