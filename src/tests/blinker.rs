use std::thread;

use crossbeam_channel::{Receiver, Sender};

use crate::{Component, Ctx, Element, Tracked};

use super::Text;

pub struct Blinker {
    state: bool,
    duration: u64,
    tx: Sender<()>,
    rx: Receiver<()>,
}

impl Component for Blinker {
    type Props = u64;

    fn render(&self, _: &Self::Props, _: Ctx<Self>) -> Vec<Element> {
        if self.state {
            vec![Text::E("Yay!".into())]
        } else {
            vec![Text::E("Nay!".into())]
        }
    }

    fn new(props: &Self::Props) -> Self {
        let (tx, rx) = crossbeam_channel::bounded(1);
        Self {
            state: false,
            duration: *props,
            tx,
            rx,
        }
    }

    fn post_update(
        state: Tracked<Self>,
        old_props: &Self::Props,
        new_props: &Self::Props,
        ctx: Ctx<Self>,
    ) {
        if old_props != new_props {
            let rx = state.rx.clone();
            let duration = state.duration;
            state.tx.send(()).unwrap();
            thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(duration));
                ctx.mutate_state(|state| {
                    state.state = !state.state;
                });
                if rx.try_recv().is_ok() {
                    break;
                }
            });
        }
    }

    fn post_mount(state: Tracked<Self>, ctx: Ctx<Self>) {
        let rx = state.rx.clone();
        let duration = state.duration;
        thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(duration));
            ctx.mutate_state(|state| {
                state.state = !state.state;
            });
            if rx.try_recv().is_ok() {
                break;
            }
        });
    }

    fn unmount(self) {
        self.tx.send(()).unwrap();
    }
}
