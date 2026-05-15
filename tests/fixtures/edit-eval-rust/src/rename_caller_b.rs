use crate::rename_target;

pub fn use_b() -> u32 {
    rename_target::target_fn() + 2
}
