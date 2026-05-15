use crate::rename_target::target_fn;

pub fn use_a() -> u32 {
    target_fn() + 1
}
