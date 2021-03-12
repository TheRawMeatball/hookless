use std::thread;

use crossbeam_channel::{Receiver, Sender};

use crate::{Component, Ctx, Tracked};

use super::Text;

pub struct Counter(usize, Sender<()>, Receiver<()>);

impl Component for Counter {
    type Props = ();

    fn render(&self, _: &Self::Props, _: crate::Ctx<Self>) -> Vec<crate::Element> {
        vec![Text::E(format!("{} seconds since creation!", self.0))]
    }

    fn new(_: &Self::Props) -> Self {
        let (tx, rx) = crossbeam_channel::bounded(1);
        Self(0, tx, rx)
    }

    fn post_update(
        state: Tracked<Self>,
        old_props: &Self::Props,
        new_props: &Self::Props,
        ctx: Ctx<Self>,
    ) {
        if old_props != new_props {
            let rx = state.2.clone();
            (*state).1.send(()).unwrap();
            thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                ctx.mutate_state(|state| {
                    state.0 += 1;
                });
                if rx.try_recv().is_ok() {
                    break;
                }
            });
        }
    }

    fn post_mount(state: Tracked<Self>, ctx: Ctx<Self>) {
        let rx = state.2.clone();
        thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            ctx.mutate_state(|state| {
                state.0 += 1;
            });
            if rx.try_recv().is_ok() {
                break;
            }
        });
    }
}