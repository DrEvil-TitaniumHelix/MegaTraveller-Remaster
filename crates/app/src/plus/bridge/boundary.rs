static DEV_BRIDGE_MARKER: &[u8] = b"MT_DEV_ONLY_BRIDGE_IMPLEMENTATION_V1";

#[inline(never)]
pub(super) fn dev_only() {
    std::hint::black_box(DEV_BRIDGE_MARKER);
}
