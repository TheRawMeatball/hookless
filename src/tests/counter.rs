use std::thread;

use crossbeam_channel::Sender;

use crate::{Component, Ctx, Fctx};

use super::Text;

pub struct Counter(usize, Sender<()>);

impl Component for Counter {
    type Props = ();

    fn render(&self, _: &Self::Props, _: crate::Ctx<Self>) -> Vec<crate::Element> {
        vec![Text::E(format!("{} seconds since creation!", self.0))]
    }

    fn new(_: &Self::Props, ctx: Ctx<Self>) -> Self {
        let (tx, rx) = crossbeam_channel::bounded(1);
        thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            ctx.mutate_state(|state| {
                state.0 += 1;
            });
            if rx.try_recv().is_ok() {
                break;
            }
        });
        Self(0, tx)
    }

    fn unmount(self) {
        self.1.send(()).unwrap();
    }
}

pub fn fnc_counter(ctx: Fctx) {
    let (state, state_setter) = ctx.use_state(|| 0);
    ctx.use_effect(Some(()), || {
        let (tx, rx) = crossbeam_channel::bounded(1);
        thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            state_setter.set(|state| {
                *state += 1;
            });
            if rx.try_recv().is_ok() {
                break;
            }
        });
        move || tx.send(()).unwrap()
    });

    ctx.render(|| vec![Text::E(format!("{} seconds since creation!", state))]);
}
