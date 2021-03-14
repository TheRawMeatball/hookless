mod blinker;
mod counter;


use super::*;
use std::fmt::Display;

use counter::*;
use blinker::*;

#[derive(Default)]
struct DemoDom {
    counter: u64,
    roots: Vec<PrimitiveId>,
    dom: HashMap<PrimitiveId, (Primitive, Vec<PrimitiveId>)>,
}

impl Display for DemoDom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "The Dom")?;
        writeln!(f, "=======")?;
        fn recursor(
            f: &mut std::fmt::Formatter<'_>,
            element: PrimitiveId,
            nest_level: i32,
            dom: &HashMap<PrimitiveId, (Primitive, Vec<PrimitiveId>)>,
        ) -> std::fmt::Result {
            let (primitive, children) = if let Some(v) = dom.get(&element) {
                v
            } else {
                return Ok(());
            };
            for _ in 0..=nest_level {
                write!(f, "|>")?;
            }
            match primitive {
                Primitive::Text(text) => writeln!(f, "{}", text)?,
                Primitive::Panel => writeln!(f, "[Fancy Panel]")?,
            }
            for child in children.iter().rev() {
                recursor(f, *child, nest_level + 1, dom)?;
            }
            Ok(())
        }

        for (i, root) in self.roots.iter().enumerate() {
            writeln!(f, "Begin root {}", i + 1)?;
            recursor(f, *root, 0, &self.dom)?;
            writeln!(f, "End root {}", i + 1)?;
        }

        Ok(())
    }
}

impl Dom for DemoDom {
    fn diff_primitive(&mut self, old: PrimitiveId, new: Primitive) {
        self.dom.get_mut(&old).unwrap().0 = new;
    }
    fn remove(&mut self, id: PrimitiveId) {
        self.dom.remove(&id);
        self.roots.retain(|v| *v != id);
    }

    fn mount_as_child(&mut self, primitive: Primitive, parent: Option<PrimitiveId>) -> PrimitiveId {
        let id = PrimitiveId(self.counter);
        self.counter += 1;
        self.dom.insert(id, (primitive, Vec::new()));
        if let Some(pid) = parent {
            self.dom.get_mut(&pid).unwrap().1.push(id);
        } else {
            self.roots.push(id);
        }
        id
    }

    fn get_sub_list(&mut self, id: PrimitiveId) -> (PrimitiveId, &mut dyn Dom) {
        (id, self)
    }
}

#[test]
fn demo() {
    let mut dom = DemoDom::default();
    println!("{:?}", std::any::TypeId::of::<()>());
    let mut context = Context::new(
        Panel::E(vec![fnc_counter.e(()), fnc_blinker.e((3,)), Blinker::E(5)]),
        &mut dom,
    );
    loop {
        if context.rx.len() > 0 {
            context.process_messages(&mut dom);
            println!("{}", &dom);
        }
    }
}

pub struct Text;

impl Component for Text {
    type Props = String;

    fn render(&self, props: &Self::Props, _: Ctx<Self>) -> Vec<Element> {
        vec![Element::Primitive(Primitive::Text(props.clone()), vec![])]
    }

    fn new(_: &Self::Props, _: Ctx<Self>) -> Self {
        Self
    }
}

pub struct Panel;

impl Component for Panel {
    type Props = Vec<Element>;

    fn render(&self, props: &Self::Props, _: Ctx<Self>) -> Vec<Element> {
        vec![Element::Primitive(Primitive::Panel, props.clone())]
    }

    fn new(_: &Self::Props, _: Ctx<Self>) -> Self {
        Self
    }
}
