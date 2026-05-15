//! Trait `Worker::run` with three impls. `impl_alpha::run` calls
//! `impl_beta::run` via a `Worker` reference (trait dispatch).
//! Does callees crossing trait dispatch resolve?

#![allow(dead_code)]

pub trait Worker { fn run(&self); }

pub struct Alpha;
pub struct Beta;
pub struct Gamma;

impl Worker for Alpha {
    fn run(&self) {
        let b = Beta;
        let w: &dyn Worker = &b;
        w.run();
    }
}

impl Worker for Beta {
    fn run(&self) {
        let g = Gamma;
        g.run();
    }
}

impl Worker for Gamma {
    fn run(&self) {}
}
