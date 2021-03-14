use std::thread;

use crossbeam_channel::Sender;

use crate::{Component, Ctx, Element, Fctx, Tracked};

use super::Text;

pub struct Blinker {
    state: bool,
    duration: u64,
    tx: Sender<()>,
}

impl Component for Blinker {
    type Props = u64;

    fn render(&self, _: &Self::Props, _: Ctx<Self>) -> Vec<Element> {
        if self.state {
            vec![Text::E(format!("Yay! - Period = {}", self.duration))]
        } else {
            vec![Text::E(format!("Nay! - Period = {}", self.duration))]
        }
    }

    fn new(props: &Self::Props, ctx: Ctx<Self>) -> Self {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let duration = *props;
        thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(duration));
            ctx.mutate_state(|state| {
                state.state = !state.state;
            });
            if rx.try_recv().is_ok() {
                break;
            }
        });
        Self {
            state: false,
            duration: *props,
            tx,
        }
    }

    fn post_update(
        mut state: Tracked<Self>,
        old_props: &Self::Props,
        new_props: &Self::Props,
        ctx: Ctx<Self>,
    ) {
        if old_props != new_props {
            state.tx.send(()).unwrap();
            let (tx, rx) = crossbeam_channel::bounded(1);
            state.tx = tx;
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
    }

    fn unmount(self) {
        self.tx.send(()).unwrap();
    }
}

pub fn fnc_blinker(ctx: Fctx, period: &u64) {
    let (is_on, set_is_on) = ctx.use_state(|| false);
    let period = *period;
    ctx.use_effect(Some(period), move || {
        let (tx, rx) = crossbeam_channel::bounded(1);
        thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(period));
            set_is_on.set(|state| {
                *state = !*state;
            });
            if rx.try_recv().is_ok() {
                break;
            }
        });
        move || tx.send(()).unwrap()
    });
    ctx.render(|| {
        if *is_on {
            vec![Text::E(format!("Yay! - Period = {}", period))]
        } else {
            vec![Text::E(format!("Nay! - Period = {}", period))]
        }
    });
}
